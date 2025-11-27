// ----------------------------
// sender.rs
// ----------------------------

// Sender responsibilities:
// - Build a GStreamer pipeline with rtpbin and udpsink for RTP and RTCP.
// - Attach an RTCP udpsrc to capture outgoing RTCP SRs so we can parse them.
// - When an RTCP SR is observed, compute/extract the NTP->RTP mapping and
//   broadcast it to all connected TCP control clients.
// - Run a persistent TCP server that accepts multiple clients and streams JSON
//   messages to them as SRs arrive and periodic heartbeats.


// Note: This sender implementation intentionally duplicates RTCP via another
// udpsrc bound to the RTCP port that rtpbin uses so we can observe SRs. In
// production you'd either get this from rtpbin signals, or use a separate
// monitoring socket.

use gst::prelude::*;
use gst::{Element, ElementFactory};
use gst_rtsp_server::RTSPServer; // optional if adding RTSP later
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{BufWriter, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const RTP_CLOCK_RATE: u32 = 90000;
const NTP_UNIX_OFFSET: u64 = 2_208_988_800;

#[derive(Debug)]
pub struct SenderArgs {
    //"239.255.12.34" this should be this machine's ip address
    multicast: String,
    //5004)]
    port: u32,
    input_file: Option<String>,
    //"0.0.0.0:9000"
    control_listen: String,
    /// Try to use vaapi hardware encoder
    use_vaapi: bool,
    /// Try to use Nvidia NVENC hardware encoder
    use_nvenc: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
enum ControlMessage {
    #[serde(rename = "sr")]
    Sr {
        ntp_seconds: u32,
        ntp_fraction: u32,
        rtp_timestamp: u32,
        clock_rate: u32,
        // optional: start_at timestamp to instruct receivers to begin playback
        start_at_ntp_seconds: Option<u64>,
        start_at_ntp_fraction: Option<u32>,
    },
    #[serde(rename = "heartbeat")]
    Heartbeat { ntp_seconds: u32, ntp_fraction: u32 },
}

fn system_time_to_ntp() -> (u32, u32) {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let unix_secs = now.as_secs();
    let unix_nanos = now.subsec_nanos();
    let ntp_secs = unix_secs + NTP_UNIX_OFFSET;
    let fraction = ((unix_nanos as u128) * (1u128 << 32) / 1_000_000_000u128) as u32;
    (ntp_secs as u32, fraction)
}

fn choose_video_encoder(use_vaapi: bool, use_nvenc: bool) -> String {
    if use_nvenc {
        if ElementFactory::find("nvh264enc").is_some() {
            return "nvh264enc".to_string();
        }
        eprintln!("nvh264enc not found, falling back");
    }
    if use_vaapi {
        if ElementFactory::find("vaapih264enc").is_some() {
            return "vaapih264enc".to_string();
        }
        eprintln!("vaapih264enc not found, falling back");
    }
    "x264enc".to_string()
}

fn start_control_server(listen_addr: String, sender_state: Arc<Mutex<Vec<ControlMessage>>>) {
    thread::spawn(move || {
        let listener = TcpListener::bind(&listen_addr).expect("Failed to bind control server");
        listener
            .set_nonblocking(true)
            .expect("Cannot set non-blocking");
        println!("Persistent control server listening on {}", listen_addr);

        let mut clients: Vec<TcpStream> = Vec::new();

        loop {
            // Accept new clients (non-blocking)
            match listener.accept() {
                Ok((stream, addr)) => {
                    println!("Control client connected: {}", addr);
                    stream
                        .set_nonblocking(false)
                        .expect("Failed to set stream blocking mode");
                    clients.push(stream);
                }
                //Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}w
                Err(e) => eprintln!("Control server accept error: {}", e),
            }

            // Broadcast latest messages to clients
            let msgs = sender_state.lock().unwrap().clone();
            if !msgs.is_empty() {
                let mut to_remove = vec![];
                for (i, client) in clients.iter_mut().enumerate() {
                    let mut writer = BufWriter::new(client);
                    for m in &msgs {
                        if let Ok(s) = serde_json::to_string(m) {
                            if let Err(e) = writer.write_all((s + "
").as_bytes()) {
                                eprintln!("Write to client failed: {}", e);
                                to_remove.push(i);
                                break;
                            }
                        }
                    }
                    if let Err(e) = writer.flush() {
                        eprintln!("Flush to client failed: {}", e);
                        to_remove.push(i);
                    }
                }
                // Remove failed clients
                for idx in to_remove.into_iter().rev() {
                    clients.swap_remove(idx);
                }
                // Clear messages after broadcasting
                sender_state.lock().unwrap().clear();
            }

            // Send a lightweight heartbeat occasionally even if no SRs
            let (ntp_s, ntp_f) = system_time_to_ntp();
            let heartbeat = ControlMessage::Heartbeat { ntp_seconds: ntp_s, ntp_fraction: ntp_f };
            let hb_json = serde_json::to_string(&heartbeat).unwrap() + "
";
            clients.retain(|client| {
                if let Err(e) = client.write_all(hb_json.as_bytes()) {
                    eprintln!("Heartbeat write failed: {}", e);
                    return false;
                }
                true
            });

            thread::sleep(Duration::from_millis(500));
        }
    });
}

fn parse_rtcp_sr_packet(packet: &[u8]) -> Option<(u32, u32, u32)> {
    // Very minimal RTCP SR parser: look for RTCP SR packet (pt=200) and extract
    // the sender info NTP timestamp (64 bits) and the RTP timestamp (32 bits).
    // RTCP SR header: V=2 (2 bits), P (1), RC (5), PT(8), length(16)
    // Sender info (after SSRC) contains: NTP timestamp (64), RTP timestamp (32), packet count, octet count
    // This parser assumes packet starts at RTCP packet boundary.
    if packet.len() < 28 {
        return None;
    }
    let pt = packet[1];
    if pt != 200u8 {
        return None;
    }
    // Skip header (4 bytes) + SSRC (4 bytes) = offset 8
    let ntp_secs = u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]);
    let ntp_frac = u32::from_be_bytes([packet[12], packet[13], packet[14], packet[15]]);
    let rtp_ts = u32::from_be_bytes([packet[16], packet[17], packet[18], packet[19]]);
    Some((ntp_secs, ntp_frac, rtp_ts))
}

pub fn init_sender(args: SenderArgs) -> Result<()> {
    gst::init()?;

    let encoder_name = choose_video_encoder(args.use_vaapi, args.use_nvenc);
    println!("Selected encoder: {}", encoder_name);

    // Build pipeline string with rtpbin
    // We'll create a pipeline programmatically to get access to elements
    let pipeline = gst::Pipeline::new(Some("sender-pipeline"));

    let src = if let Some(file) = &args.input_file {
        ElementFactory::make("filesrc", Some("src")).unwrap()
    } else {
        ElementFactory::make("autovideosrc", Some("src")).unwrap()
    };

    let decode = ElementFactory::make("decodebin", Some("decode")).unwrap_or_else(|| ElementFactory::make("identity", None).unwrap());

    let convert = ElementFactory::make("videoconvert", Some("convert")).unwrap();
    let encoder = ElementFactory::make(&encoder_name, Some("encoder")).unwrap();
    // Common encoder properties for x264enc fallback
    if encoder_name == "x264enc" {
        encoder.set_property_from_str("tune", "zerolatency");
        encoder.set_property("bitrate", &2000u32).ok();
        encoder.set_property_from_str("speed-preset", "superfast");
    }

    let pay = ElementFactory::make("rtph264pay", Some("pay")).unwrap();

    let rtpbin = ElementFactory::make("rtpbin", Some("rtpbin")).unwrap();

    let udpsink_rtp = ElementFactory::make("udpsink", Some("udpsink_rtp")).unwrap();
    udpsink_rtp.set_property("host", &args.multicast)?;
    udpsink_rtp.set_property("port", &(args.port as i32))?;
    udpsink_rtp.set_property("auto-multicast", &true)?;
    udpsink_rtp.set_property("sync", &false)?;
    udpsink_rtp.set_property("async", &false)?;

    let udpsink_rtcp = ElementFactory::make("udpsink", Some("udpsink_rtcp")).unwrap();
    udpsink_rtcp.set_property("host", &args.multicast)?;
    udpsink_rtcp.set_property("port", &((args.port + 1) as i32))?;
    udpsink_rtcp.set_property("auto-multicast", &true)?;
    udpsink_rtcp.set_property("sync", &false)?;
    udpsink_rtcp.set_property("async", &false)?;

    // A local UDP socket to receive RTCP sent by rtpbin (bind to port+1)
    let rtcp_socket = UdpSocket::bind(("0.0.0.0", (args.port + 1) as u16)).expect("Failed to bind RTCP monitor socket");
    rtcp_socket
        .set_nonblocking(true)
        .expect("Failed to set non-blocking RTCP socket");

    // Link elements: src ! decodebin -> convert -> encoder -> pay -> rtpbin.send_rtp_sink_0
    pipeline.add_many(&[&src, &decode, &convert, &encoder, &pay, &rtpbin, &udpsink_rtp, &udpsink_rtcp])?;

    // Connect dynamic pads for decodebin
    let pipeline_weak = pipeline.downgrade();
    decode.connect("pad-added", false, move |args| {
        let pipeline = match pipeline_weak.upgrade() {
            Some(p) => p,
            None => return None,
        };
        let srcpad = args[1].get::<gst::Pad>().unwrap();
        let convert = pipeline.by_name("convert").unwrap();
        let sinkpad = convert.static_pad("sink").unwrap();
        srcpad.link(&sinkpad).ok();
        None
    })?;

    // Link convert -> encoder -> pay
    Element::link_many(&[&convert, &encoder, &pay])?;

    // Connect pay to rtpbin: pay.src -> rtpbin.send_rtp_sink_0
    let pay_srcpad = pay.static_pad("src").unwrap();
    let rtpbin_sinkpad = rtpbin.get_request_pad("send_rtp_sink_0").unwrap();
    pay_srcpad.link(&rtpbin_sinkpad)?;

    // rtpbin src for RTP -> udpsink_rtp
    let rtpbin_srcpad = rtpbin.get_static_pad("send_rtp_src_0").unwrap();
    let udpsink_rtp_sinkpad = udpsink_rtp.static_pad("sink").unwrap();
    rtpbin_srcpad.link(&udpsink_rtp_sinkpad)?;

    // rtpbin src for RTCP -> udpsink_rtcp
    let rtpbin_rtcp_srcpad = rtpbin.get_static_pad("send_rtcp_src_0").unwrap();
    let udpsink_rtcp_sinkpad = udpsink_rtcp.static_pad("sink").unwrap();
    rtpbin_rtcp_srcpad.link(&udpsink_rtcp_sinkpad)?;

    // Start pipeline
    pipeline.set_state(gst::State::Playing)?;

    // Shared sender state where parsed SRs will be appended for broadcasting
    let sender_state: Arc<Mutex<Vec<ControlMessage>>> = Arc::new(Mutex::new(vec![]));
    start_control_server(args.control_listen.clone(), sender_state.clone());

    // Monitor RTCP socket for SR packets and when found, parse and push to sender_state
    // This loop also periodically broadcasts heartbeat messages via sender_state
    thread::spawn(move || {
        let mut buf = [0u8; 1500];
        loop {
            match rtcp_socket.recv(&mut buf) {
                Ok(sz) => {
                    if let Some((ntp_s, ntp_f, rtp_ts)) = parse_rtcp_sr_packet(&buf[..sz]) {
                        // Compose ControlMessage::Sr and push to state
                        let msg = ControlMessage::Sr {
                            ntp_seconds: ntp_s,
                            ntp_fraction: ntp_f,
                            rtp_timestamp: rtp_ts,
                            clock_rate: RTP_CLOCK_RATE,
                            start_at_ntp_seconds: Some((SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 5) + NTP_UNIX_OFFSET),
                            start_at_ntp_fraction: Some(ntp_f),
                        };
                        sender_state.lock().unwrap().push(msg);
                    }
                }
                //Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // no packet available
                //}
                Err(e) => eprintln!("RTCP monitor socket error: {}", e),
            }
            thread::sleep(Duration::from_millis(50));
        }
    });

    // Main bus loop for pipeline errors and EOS
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
