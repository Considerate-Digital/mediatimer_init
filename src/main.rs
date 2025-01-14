use std::{
    sync::{Arc, Mutex, LazyLock},
    io,
    thread,
    time::Duration,
    path::{Path, PathBuf},
    env,
    error::Error,
    process,
    process::{
        Command,
        Child
    }
};
use clokwerk::{
    Scheduler,
    Interval,
    Job
};

use regex::Regex;

#[derive(Debug,Clone, Copy)]
enum ProcType {
    Media,
    Browser,
    Executable,
}

#[derive(Debug, Clone, Copy)]
enum Autoloop {
    Yes,
    No
}
#[derive(Debug, Clone)]
enum Weekday {
    Monday(Vec<(String, String)>),
    Tuesday(Vec<(String, String)>),
    Wednesday(Vec<(String, String)>),
    Thursday(Vec<(String, String)>),
    Friday(Vec<(String, String)>),
    Saturday(Vec<(String, String)>),
    Sunday(Vec<(String, String)>),
}

type Timings = Vec<Weekday>;

/// This program runs one task at custom intervals. The task can also be looped.
/// Commonly this is used for playing media files at certain times.
/// The Task struct is the main set of instructions that are written out into an env file to be 
/// interpreted in future by the init program.
#[derive(Debug)]
struct Task {
    proc_type: ProcType,
    auto_loop: Autoloop,
    timings: Timings,
    file: PathBuf
}

impl Task {
    fn new(proc_type: ProcType, auto_loop: Autoloop, timings: Timings, file: PathBuf) -> Self {
        Task {
            proc_type,
            auto_loop,
            timings,
            file
        }
    }
    fn set_loop(&mut self, auto_loop: Autoloop) {
        self.auto_loop = auto_loop;
    }
    fn set_proc_type(&mut self, p_type: ProcType) {
        self.proc_type = p_type;
    }
    fn set_weekday(&mut self, wd: Weekday) {
        self.timings.push(wd);
    }
}

enum Usb {
    SDA1,
    SDA2,
    SDA3,
    SDA4,
    SDB1,
    SDB2,
    SDB3,
    SDB4,
    SDC1,
    SDC2,
    SDC3,
    SDC4,
    UNKNOWN
}

impl Usb {
    fn as_str(&self) -> &'static str {
        match self {
            Usb::SDA1 => "sda1", 
            Usb::SDA2 => "sda2", 
            Usb::SDA3 => "sda3", 
            Usb::SDA4 => "sda4",
            Usb::SDB1 => "sdb1", 
            Usb::SDB2 => "sdb2", 
            Usb::SDB3 => "sdb3",
            Usb::SDB4 => "sdb4",
            Usb::SDC1 => "sdc1",
            Usb::SDC2 => "sdc2",
            Usb::SDC3 => "sdc3",
            Usb::SDC4 => "sdc4",
            Usb::UNKNOWN => ""
        }
    }
}


fn find_mount_drives() -> Result<(), Box<dyn Error>> {
    println!("Finding and mounting drives");
    // check with usbs are available 
    let all_drives = Command::new("lsblk")
        .arg("-l")
        .arg("-o")
        .arg("NAME,HOTPLUG")
        .output()
        .expect("some drives");
    
    let all_drives_string = String::from_utf8_lossy(&all_drives.stdout);
    
    for line in all_drives_string.lines() {
        let re = Regex::new(r"sd[a,b,c][1-4]").unwrap();
        if re.is_match(line) {
            let drive_info = line.split(' ')
                .filter(|d| *d != "" )
                .collect::<Vec<_>>();
                if drive_info[1] == "1" { 
                    // unmount the drive before going further
                    let unmount_com = Command::new("umount")
                        .arg("/dev/".to_owned() + drive_info[0])
                        .output()
                        .expect("Failed to unmount usb drive");

                    println!("{:?}", unmount_com);
                    let drive = match drive_info[0] {
                        "sda1" => Usb::SDA1,
                        "sda2" => Usb::SDA2,
                        "sda3" => Usb::SDA3,
                        "sda4" => Usb::SDA4,
                        "sdb1" => Usb::SDB1,
                        "sdb2" => Usb::SDB2,
                        "sdb3" => Usb::SDB3,
                        "sdb4" => Usb::SDB4,
                        "sdc1" => Usb::SDC1,
                        "sdc2" => Usb::SDC2,
                        "sdc3" => Usb::SDC3,
                        "sdc4" => Usb::SDC4,
                        &_ => Usb::UNKNOWN

                    };
                    mount_usb(drive)?;
                }
        }
    }
    Ok(())
}

fn mount_usb(drive: Usb) -> Result<(), Box<dyn Error>> {

    let mnt_dir: String = match drive {
        Usb::SDA1 => format!("usb_{}", Usb::SDA1.as_str()),
        Usb::SDA2 => format!("usb_{}", Usb::SDA2.as_str()),
        Usb::SDA3 => format!("usb_{}", Usb::SDA3.as_str()),
        Usb::SDA4 => format!("usb_{}", Usb::SDA4.as_str()),
        Usb::SDB1 => format!("usb_{}", Usb::SDB1.as_str()),
        Usb::SDB2 => format!("usb_{}", Usb::SDB2.as_str()),
        Usb::SDB3 => format!("usb_{}", Usb::SDB3.as_str()),
        Usb::SDB4 => format!("usb_{}", Usb::SDB4.as_str()),
        Usb::SDC1 => format!("usb_{}", Usb::SDC1.as_str()),
        Usb::SDC2 => format!("usb_{}", Usb::SDC2.as_str()),
        Usb::SDC3 => format!("usb_{}", Usb::SDC3.as_str()),
        Usb::SDC4 => format!("usb_{}", Usb::SDC4.as_str()),
        Usb::UNKNOWN => "".to_string()

    };
    let drive_name = match drive {
        Usb::SDA1 => Usb::SDA1.as_str(), 
        Usb::SDA2 => Usb::SDA2.as_str(), 
        Usb::SDA3 => Usb::SDA3.as_str(), 
        Usb::SDA4 => Usb::SDA4.as_str(),
        Usb::SDB1 => Usb::SDB1.as_str(), 
        Usb::SDB2 => Usb::SDB2.as_str(), 
        Usb::SDB3 => Usb::SDB3.as_str(),
        Usb::SDB4 => Usb::SDB4.as_str(),
        Usb::SDC1 => Usb::SDC1.as_str(),
        Usb::SDC2 => Usb::SDC2.as_str(),
        Usb::SDC3 => Usb::SDC3.as_str(),
        Usb::SDC4 => Usb::SDC4.as_str(),
        Usb::UNKNOWN => ""
    };
    let mount_drive = Command::new("mount")
            .arg("/dev/".to_owned() + drive_name)
            // tell mount to make the target dir
            .arg("-o")
            .arg("rw,x-mount.mkdir")
            .arg("/mnt/".to_owned() + &mnt_dir)
            .output()
            .expect("failed to mount");

    Ok(())
}


fn to_weekday(value: String, day: Weekday) -> Result<Weekday, Box<dyn Error>> {
    let string_vec: Vec<String> = value.as_str().split(",").map(|x| x.to_string()).collect(); 
    let mut day_schedule = Vec::new();
    for time in string_vec.iter() {
        let start_end = time.as_str()
            .split("-")
            .map(|x| x.to_string())
            .collect::<Vec<String>>();
        let start = start_end[0].clone();
        let end = start_end[1].clone();
        day_schedule.push((start, end));
    }
    match day {
       Weekday::Monday(_) =>  Ok(Weekday::Monday(day_schedule)),
       Weekday::Tuesday(_) => Ok(Weekday::Tuesday(day_schedule)),
       Weekday::Wednesday(_) => Ok(Weekday::Wednesday(day_schedule)),
       Weekday::Thursday(_) => Ok(Weekday::Thursday(day_schedule)),
       Weekday::Friday(_) => Ok(Weekday::Friday(day_schedule)),
       Weekday::Saturday(_) => Ok(Weekday::Saturday(day_schedule)),
       Weekday::Sunday(_) => Ok(Weekday::Sunday(day_schedule))
    }
}
static mut RUNNING_TASK: LazyLock<Mutex<Vec<Child>>> = LazyLock::new(|| Mutex::new(Vec::new()));

fn run_task(task: Arc<Mutex<Task>>) {
    println!("{:?}", task);
    let looper = match task.lock().unwrap().auto_loop {
        Autoloop::Yes => Autoloop::Yes,
        Autoloop::No => Autoloop::No
    };
    let file = task.lock().unwrap().file.clone();
    let proc_type = match task.lock().unwrap().proc_type {
        ProcType::Media => ProcType::Media,
        ProcType::Browser => ProcType::Browser,
        ProcType::Executable => ProcType::Executable
    };
    println!("{:?}", task);
    match task.lock().unwrap().proc_type {
        ProcType::Media => {
            let loopy = match looper {
                Autoloop::Yes => "-L",
                Autoloop::No => "",
            };
            let child = Command::new("cvlc")
                .arg("-f")
                .arg(loopy)
                .arg("--no-video-title-show")
                .arg(file)
                .spawn().expect("no child");
            unsafe {
                RUNNING_TASK.lock().unwrap().push(child);
            }
        },
        ProcType::Browser => {
            let child = Command::new("chromium")
                .arg(task.lock().unwrap().file.clone())
                .arg("--start-fullscreen")
                .arg("--start-maximized")
                .spawn().expect("no child");
            unsafe {
                RUNNING_TASK.lock().unwrap().push(child);
            }

        },
        ProcType::Executable => {
            let mut command = String::from(".");
            if let Some(file_str) = task.lock().unwrap().file.to_str() {
                command.push_str(file_str);

                let child = Command::new(&command)
                    .spawn().expect("no child");

                unsafe {
                    RUNNING_TASK.lock().unwrap().push(child);
                }
            }

        }
    }
}

fn stop_task() {
    unsafe {
        let mut task = RUNNING_TASK.lock().unwrap();
        task[0].kill().expect("command could not be killed");

        // only one task is run at a time, so it is safe to pop.
        RUNNING_TASK.lock().unwrap().pop();
    }
}

fn main() -> Result<(), Box<dyn Error>> {


    // check which usbs are mounted

    let _mount_drives = find_mount_drives()?;
 

    // use this dir for testing
    //
    let mut env_dir_path = PathBuf::new();

    if let Some(dir) = home::home_dir() {
        env_dir_path.push(dir);
    } else {
        env_dir_path.push("/home/");
    }
    env_dir_path.push("medialoop_config/vars");

    if let Err(e) = dotenvy::from_path(env_dir_path.as_path()) {
        eprintln!("Please run medialoop, to setup this program");
        process::exit(1)
    }
   
    let mut file = PathBuf::new();
    let mut proc_type = String::with_capacity(10);
    let mut auto_loop = Autoloop::No;
    let mut schedule = false;
    let mut timings: Vec<Weekday> = Vec::with_capacity(7);
    let mut monday: Weekday = Weekday::Monday(Vec::with_capacity(2));
    let mut tuesday: Weekday = Weekday::Tuesday(Vec::with_capacity(2));
    let mut wednesday: Weekday = Weekday::Wednesday(Vec::with_capacity(2));
    let mut thursday: Weekday = Weekday::Thursday(Vec::with_capacity(2));
    let mut friday: Weekday = Weekday::Friday(Vec::with_capacity(2));
    let mut saturday: Weekday = Weekday::Saturday(Vec::with_capacity(2));
    let mut sunday: Weekday = Weekday::Sunday(Vec::with_capacity(2));

    for (key, value) in env::vars() {
        match key.as_str() {
            "ML_PROCTYPE" => proc_type.push_str(&value),
            "ML_AUTOLOOP" => auto_loop = match value.as_str() {
                "true" => Autoloop::Yes,
                "false" => Autoloop::No,
                &_ => Autoloop::No
            },
            "ML_FILE" => file.push(value.as_str()),
            "ML_SCHEDULE" => match value.as_str() {
                "true" => schedule = true,
                "false" => schedule = false,
                &_ => schedule = false
            },
            "ML_MONDAY" => monday = to_weekday(value, Weekday::Monday(Vec::new()))?,
            "ML_TUESDAY" => tuesday = to_weekday(value, Weekday::Tuesday(Vec::new()))?,
            "ML_WEDNESDAY" => wednesday = to_weekday(value, Weekday::Wednesday(Vec::new()))?,
            "ML_THURSDAY" => thursday = to_weekday(value, Weekday::Thursday(Vec::new()))?,
            "ML_FRIDAY" => friday = to_weekday(value, Weekday::Friday(Vec::new()))?,
            "ML_SATURDAY" => saturday = to_weekday(value, Weekday::Saturday(Vec::new()))?,
            "ML_SUNDAY" => sunday = to_weekday(value, Weekday::Sunday(Vec::new()))?,
            _ => {}
        }
    }

    timings = vec![monday, tuesday, wednesday, thursday, friday, saturday, sunday]; 
    

    let timings_clone = timings.clone();

    // convert the proc type to enum
    let proc_type = match proc_type.to_lowercase().as_str() {
        "media" => ProcType::Media,
        "browser" => ProcType::Browser,
        "executable" => ProcType::Executable,
        &_ => ProcType::Media
    };

    let task: Arc<Mutex<Task>> = Arc::new(Mutex::new(Task::new(proc_type, auto_loop, timings, file)));

    
    // set up scheduler
    let mut scheduler = Scheduler::new();
    
    if schedule == true {
       // use the full scheduler
       println!("using the full scheduler");
       for day in timings_clone.iter() {
           let day_name = match day {
                Weekday::Monday(_) => Interval::Monday,
                Weekday::Tuesday(_) => Interval::Tuesday,
                Weekday::Wednesday(_) => Interval::Wednesday,
                Weekday::Thursday(_) => Interval::Thursday,
                Weekday::Friday(_) => Interval::Friday, 
                Weekday::Saturday(_) => Interval::Saturday, 
                Weekday::Sunday(_) => Interval::Sunday 
      
           };
           let timing_vec = match day {
                Weekday::Monday(t) => t,
                Weekday::Tuesday(t) => t,
                Weekday::Wednesday(t) => t,
                Weekday::Thursday(t) => t,
                Weekday::Friday(t) => t, 
                Weekday::Saturday(t) => t, 
                Weekday::Sunday(t) => t 
           };

            for timing in timing_vec.iter() {
               let task_clone = Arc::clone(&task);
               println!("{:?}", timing);
               scheduler.every(day_name)
                   .at(&timing.0)
                   .run(move || run_task(task_clone.clone()));

                scheduler.every(day_name)
                    .at(&timing.1)
                    .run(|| stop_task());
            }
       }
       loop {
           scheduler.run_pending();
           thread::sleep(Duration::from_millis(10));
       }
   } else {
       // run the task now
       println!("running the task now");
       let task_clone = Arc::clone(&task); 
       let task_aut = run_task(task_clone);
       println!("{:?}", task_aut);
   }

    Ok(())
}
