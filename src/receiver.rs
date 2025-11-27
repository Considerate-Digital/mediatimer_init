// ----------------------------
// receiver.rs
// ----------------------------

// Receiver responsibilities:
// - Connect persistently to sender control server; apply SR mappings as they arrive.
// - Optionally listen to RTCP packets on the receiver side to verify SRs and adjust
//   local playout timing (this example shows a simple verification step).
// - Keep pipeline PAUSED for preroll and switch to PLAYING at the agreed timestamp.

use serde_json::Value;
use std::io::BufRead;
use std::io::BufReader;
use std::net::TcpStream as StdTcpStream;

#[derive(Debug)]
pub struct ReceiverArgs {
    //"239.255.12.34")]
    //multicast: String,
    //5004
    port: u32,
    //#[arg(long, default_value = "autovideosink")]
    display_sink: String,
    //#[arg(long, default_value = "127.0.0.1:9000")]
    control_addr: String,
}

#[derive(Debug, Clone)]
struct SrInfo {
    ntp_seconds: u64, // absolute NTP seconds since 1900
    ntp_fraction: u32,
    rtp_timestamp: u32,
    clock_rate: u32,
}

fn ntp_now() -> (u64, u32) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let unix_secs = now.as_secs();
    let unix_nanos = now.subsec_nanos();
    let ntp_secs = unix_secs + NTP_UNIX_OFFSET;
    let fraction = ((unix_nanos as u128) * (1u128 << 32) / 1_000_000_000u128) as u32;
    (ntp_secs, fraction)
}

fn parse_control_line(line: &str) -> Option<ControlMessage> {
    serde_json::from_str::<ControlMessage>(line).ok()
}

fn connect_control_persistent(addr: &str) -> Result<StdTcpStream> {
    let mut last_err = None;
    for _ in 0..10 {
        match StdTcpStream::connect(addr) {
            Ok(s) => {
                s.set_nonblocking(false).ok();
                return Ok(s);
            }
            Err(e) => {
                last_err = Some(e);
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
    Err(anyhow::anyhow!(last_err.unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "connect failed"))))
}

fn init_receiver(args: ReceiverArgs) -> Result<()> {
    gst::init()?;

    println!("Connecting to control server {}...", args.control_addr);
    let stream = connect_control_persistent(&args.control_addr)?;
    let mut reader = BufReader::new(stream.try_clone()?);

    // We'll receive control messages and keep the last SR mapping
    let mut last_sr: Option<SrInfo> = None;
    let mut scheduled_start: Option<u64> = None;

    // Non-blocking thread to read control messages continuously and update last_sr
    let (tx, rx) = std::sync::mpsc::channel();
    let mut reader_clone = reader;
    thread::spawn(move || loop {
        let mut line = String::new();
        match reader_clone.read_line(&mut line) {
            Ok(0) => {
                eprintln!("Control server closed connection");
                break;
            }
            Ok(_) => {
                if let Some(msg) = parse_control_line(line.trim()) {
                    if tx.send(msg).is_err() {
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("Control read error: {}", e);
                break;
            }
        }
    });

    // Build the pipeline and set to PAUSED
    let pipeline_str = format!("udpsrc address=0.0.0.0 port={} caps=application/x-rtp,media=video,encoding-name=H264,payload=96 ! rtph264depay ! avdec_h264 ! videoconvert ! {}", args.port, args.display_sink);
    println!("Pipeline: {}", pipeline_str);
    let pipeline = gst::parse_launch(&pipeline_str)?;
    let pipeline = pipeline.downcast::<gst::Pipeline>().unwrap();
    pipeline.set_state(gst::State::Paused)?;

    // Non-blocking processing of control messages, schedule start when SR arrives
    loop {
        // Check for incoming control messages
        if let Ok(msg) = rx.try_recv() {
            match msg {
                ControlMessage::Sr { ntp_seconds, ntp_fraction, rtp_timestamp, clock_rate, start_at_ntp_seconds, start_at_ntp_fraction } => {
                    println!("Received SR: ntp={} frac={} rtp={} clk={}", ntp_seconds, ntp_fraction, rtp_timestamp, clock_rate);
                    let ntp_abs = ntp_seconds as u64; // already NTP seconds
                    last_sr = Some(SrInfo { ntp_seconds: ntp_abs, ntp_fraction, rtp_timestamp, clock_rate });
                    if let Some(start_abs) = start_at_ntp_seconds {
                        scheduled_start = Some(start_abs);
                    }
                }
                ControlMessage::Heartbeat { ntp_seconds, ntp_fraction } => {
                    // Could be used for clock health; ignore for now
                }
            }
        }

        // If we have a scheduled start in the future, wait in loop; if it's time, start
        if let Some(start_abs) = scheduled_start {
            let (now_s, _now_f) = ntp_now();
            if now_s >= start_abs {
                println!("Scheduled start reached, setting PLAYING");
                pipeline.set_state(gst::State::Playing)?;
                break;
            }
        }

        // Sleep briefly to avoid busy loop
        thread::sleep(Duration::from_millis(50));
    }

    // After starting, also spawn a thread to listen to RTCP on the receiver's RTCP port to verify SRs
    thread::spawn(move || {
        let rtcp_port = (args.port + 1) as u16;
        if let Ok(sock) = UdpSocket::bind(("0.0.0.0", rtcp_port)) {
            sock.set_nonblocking(true).ok();
            let mut buf = [0u8; 1500];
            loop {
                match sock.recv(&mut buf) {
                    Ok(sz) => {
                        if let Some((ntp_s, ntp_f, rtp_ts)) = parse_rtcp_sr_packet(&buf[..sz]) {
                            println!("Receiver observed RTCP SR: ntp={} frac={} rtp={}", ntp_s, ntp_f, rtp_ts);
                            // Could compare to last_sr and detect drift; implement re-sync logic here
                        }
                    }
                    //Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => eprintln!("RTCP recv error: {}", e),
                }
                thread::sleep(Duration::from_millis(100));
            }
        } else {
            eprintln!("Receiver could not bind RTCP port {} for verification", rtcp_port);
        }
    });

    // Main bus loop
    let bus = pipeline.bus().expect("Pipeline without bus");
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        use gst::MessageView;
        match msg.view() {
            MessageView::Eos(..) => {
                eprintln!("End of stream");
                break;
            }
            MessageView::Error(err) => {
                eprintln!("Error from {:?}: {} ({:?})", err.src().map(|s| s.path_string()), err.error(), err.debug());
                break;
            }
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null)?;
    Ok(())
}


