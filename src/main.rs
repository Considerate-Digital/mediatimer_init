use std::{
    io,
    thread,
    time::Duration,
    path::{Path, PathBuf},
    env,
    error::Error,
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

#[derive(Debug)]
enum ProcType {
    Media,
    Browser,
    Executable,
}

#[derive(Debug)]
enum Autoloop {
    Yes,
    No
}
#[derive(Debug)]
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
/*
fn mount_usb() -> Result<(), Box<dyn Error>> {
    
    Ok(())
}
*/


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
       Weekday::Sunday(_) => Ok(Weekday::Sunday(day_schedule)),
    }
}
use std::sync::{LazyLock, Mutex};
static mut RUNNING_TASK: LazyLock<Mutex<Vec<Child>>> = LazyLock::new(|| Mutex::new(Vec::new()));

fn run_task(proc_type: Proctype, auto_loop: Autoloop, file: &PathBuf) {
    match proc_type {
        ProcType::Media => {
            let loopy = match auto_loop {
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
            let output = Command::new("firefox")
                .arg(file)
                .arg("&")
                .arg("xdotool")
                .arg("search")
                .arg("--sync")
                .arg("--onlyvisible")
                .arg("--class")
                .arg("\"FireFox\"")
                .arg("windowactivate")
                .arg("key")
                .arg("F11")
                .spawn().unwrap();

        },
        ProcType::Executable => {
            let mut command = String::from("./");
            if let Some(file_str) = file.to_str() {
                command.push_str(file_str);
                let output = Command::new(&command);
            }

        }
    }
}

fn stop_task() {
    unsafe {
        let mut task = RUNNING_TASK.lock().unwrap();
        task[0].kill().expect("command could not be killed");
        RUNNING_TASK.lock().unwrap().pop();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // use this dir for testing
    let mut env_dir_path = PathBuf::new();

    if let Some(dir) = home::home_dir() {
        env_dir_path.push(dir);
    } else {
        env_dir_path.push("/home/");
    }
    env_dir_path.push("medialoop_config/vars");

    dotenvy::from_path(env_dir_path.as_path())?;

    let usb_mounts = vec![
        ("sda1", "usb_stick_1"), 
        ("sda2", "usb_stick_1_2"), 
        ("sdb1", "usb_stick_2"),
        ("sdb2", "usb_stick_2_2"), 
        ("sdc1", "usb_stick_3"),
        ("sdc2", "usb_stick_3_3")
    ];

    // check which usbs are mounted
    /*
    let usb_drives = Command::new("grep")
        .arg("-i")
        .arg("-E")
        .arg("/dev/sd(a|b|c|d)")
        .arg("/proc/mounts")
        .output()
        .expect("failed to check for usb drives");

    println!("{:?}", usb_drives);
    */

    // check with usbs are available 
    let all_drives = Command::new("lsblk")
        .arg("-l")
        .output()
        .expect("some drives");
    let all_drives_string = String::from_utf8_lossy(&all_drives.stdout);
    //println!("{}", all_drives_string);
    let mut usb_no = 1;
    for line in all_drives_string.lines() {
        let re = Regex::new(r"sda[1-5]").unwrap();
        if re.is_match(line) {
            println!("{:?}", line);
            //mount_usb()
        }
    }

    //for 

    // setup mounts
    for (drive, name) in usb_mounts {
        /*
        let setup_dir = Command::new("mkdir")
            .arg("/mnt/".to_owned() + point)
            .output()
            .expect("failed to create dir");
        println!("{:?}", setup_dir);
        */
        /*
        let mount = Command::new("mount")
            .arg("/dev/".to_owned() + drive)
            // tell mount to make the target dir
            .arg("-o")
            .arg("rw,x-mount.mkdir")
            .arg("/mnt/".to_owned() + name)
            .output()
            .expect("failed to mount");
        println!("{:?}",mount);
        */
    }     
    
    let mut file = PathBuf::new();
    let mut proc_type = String::with_capacity(10);
    let mut auto_loop = Autoloop::No;
    /*
    let mut timings: [Weekday; 7] = [
        Weekday::Monday(Vec::new()),
        Weekday::Tuesday(Vec::new()),
        Weekday::Wednesday(Vec::new()),
        Weekday::Thursday(Vec::new()),
        Weekday::Friday(Vec::new()),
        Weekday::Saturday(Vec::new()),
        Weekday::Sunday(Vec::new()),

    ];
    */
    let mut timings: Vec<Weekday> = Vec::with_capacity(7);
    let mut monday: Weekday = Weekday::Monday(Vec::with_capacity(2));
    /*
    let mut tuesday = Vec::with_capacity(2);
    let mut wednesday = Vec::with_capacity(2);
    let mut thursday = Vec::with_capacity(2);
    let mut friday = Vec::with_capacity(2);
    let mut saturday = Vec::with_capacity(2);
    let mut sunday = Vec::with_capacity(2);
    */

    for (key, value) in env::vars() {
        match key.as_str() {
            "ML_PROCTYPE" => proc_type.push_str(&value),
            "ML_AUTOLOOP" => auto_loop = match value.as_str() {
                "true" => Autoloop::Yes,
                "false" => Autoloop::No,
                &_ => Autoloop::No
            },
            "ML_FILE" => file.push(value.as_str()),
            "ML_MONDAY" => monday = to_weekday(value, Weekday::Monday(Vec::new()))?,
            _ => {}
        }
    }

    
    println!("{:?}", monday);

    timings.push(monday);

    // convert the proc type to enum
    let proc_type = match proc_type.to_lowercase().as_str() {
        "media" => ProcType::Media,
        "browser" => ProcType::Browser,
        "executable" => ProcType::Executable,
        &_ => ProcType::Media
    };

    let task: Task = Task::new(proc_type, auto_loop, timings, file);
   // check if there are any day variables set 
   let Weekday::Monday(monday_vec) = &task.timings[0] else { todo!() };
    
    let mut control_scheduler = Scheduler::new();
    // set up scheduler
    let mut scheduler = Scheduler::new();

    let thread_handle = scheduler.watch_thread(Duration::from_millis(1000));

   if monday_vec.len() > 1 {
       // use the full scheduler
       println!("using the full scheduler");
       for day in task.timings.iter() {
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
               scheduler.every(day_name)
                   .at(&timing.0)
                   .run(|| run_task());

                scheduler.every(day_name)
                    .at(&timing.1)
                    .run(|| stop_task());
            }
       }
   } else {
       // run the task now
       println!("running the task now");
       run_task(task);
   }

    Ok(())
}
