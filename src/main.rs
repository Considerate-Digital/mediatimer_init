use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
    path::{PathBuf},
    env,
    error::Error,
    process,
    process::{
        Command,
        Child
    },
    ops::Deref,
};

use clokwerk::{
    Scheduler,
    Interval,
    Job
};

use regex::Regex;
use whoami;

mod mount;
use crate::mount::identify_mounted_drives;

mod background;

mod error;
use crate::error::error as display_error;
use crate::error::error_with_message as display_error_with_message;

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

type Schedule = Vec<(String, String)>;
type Timings = Vec<Weekday>;

#[derive(Display, Debug, Clone)]
pub enum Weekday {
    Monday(Schedule),
    Tuesday(Schedule),
    Wednesday(Schedule),
    Thursday(Schedule),
    Friday(Schedule),
    Saturday(Schedule),
    Sunday(Schedule),
}

impl Weekday {
    fn as_str(&self) -> &'static str {
        match self {
            Weekday::Monday(_) => "Monday",
            Weekday::Tuesday(_) => "Tuesday",
            Weekday::Wednesday(_) => "Wednesday",
            Weekday::Thursday(_) => "Thursday",
            Weekday::Friday(_) => "Friday",
            Weekday::Saturday(_) => "Saturday",
            Weekday::Sunday(_) => "Sunday"
        }
    }
    fn to_string(&self) -> String {
        match self {
            Weekday::Monday(_) => String::from("Monday"),
            Weekday::Tuesday(_) => String::from("Tuesday"),
            Weekday::Wednesday(_) => String::from("Wednesday"),
            Weekday::Thursday(_) => String::from("Thursday"),
            Weekday::Friday(_) => String::from("Friday"),
            Weekday::Saturday(_) => String::from("Saturday"),
            Weekday::Sunday(_) => String::from("Sunday")
        }
    }

    fn timings(&self) -> Schedule {
        match self {
            Weekday::Monday(schedule) => schedule.clone(),
            Weekday::Tuesday(schedule) => schedule.clone(),
            Weekday::Wednesday(schedule) => schedule.clone(),
            Weekday::Thursday(schedule) => schedule.clone(),
            Weekday::Friday(schedule) => schedule.clone(),
            Weekday::Saturday(schedule) => schedule.clone(),
            Weekday::Sunday(schedule) => schedule.clone()
        }
    }
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
    file: PathBuf,
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

#[derive(Debug)]
struct RunningTask {
    child: process::Child,
    background: bool
}

impl RunningTask {
    fn new(child: Child, background: bool) -> RunningTask {
        RunningTask {
            child,
            background
        }
    }
}


fn to_weekday(value: String, day: Weekday) -> Result<Weekday, Box<dyn Error>> {
    let mut day_schedule = Vec::new();
    if &value != "" {
        let string_vec: Vec<String> = value.as_str().split(",").map(|x| x.trim().to_string()).collect(); 
        let string_vec_test = string_vec.clone();

        // check the schedule format matches 00:00 or 00:00:00
        // move these check to the "to weekday" function
        // TODO change to updated regex
        let re = Regex::new(r"(^\d{2}:\d{2}-\d{2}:\d{2}$|^\d{2}:\d{2}:\d{2}-\d{2}:\d{2}:\d{2}$|^\d{2}:\d{2}-\d{2}:\d{2}:\d{2}$|^\d{2}:\d{2}:\d{2}-\d{2}:\d{2}$)").unwrap();
        // check the times split correctly
        let parsed_count = string_vec_test.len();  
        let string_of_times = string_vec_test.iter().map(|s| s.to_string()).collect::<String>();
        let mut re_count = 0;
        for time in string_vec_test.iter() {
            if re.is_match(&time) == true {
                re_count += 1;
            }
        }
        if parsed_count != re_count {
            println!("{}, {}", parsed_count, re_count);
            // timings do not match
            display_error_with_message("Schedule incorrectly formatted!");
            process::exit(1);
        }

        for time in string_vec.iter() {
            let start_end = time.as_str()
                .split("-")
                .map(|x| x.to_string())
                .collect::<Vec<String>>();
            let start = start_end[0].clone();
            let end = start_end[1].clone();
            day_schedule.push((start, end));
        }
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

fn run_task(task_list: Arc<Mutex<Vec<RunningTask>>>, task: Arc<Mutex<Task>>) {
    let task_list_clone = Arc::clone(&task_list);
    let _stopped_task = stop_task(task_list.clone());
    println!("Stopped previous task and trying to run new task");
    let looper = match task.lock().unwrap().auto_loop {
        Autoloop::Yes => Autoloop::Yes,
        Autoloop::No => Autoloop::No
    };
    let file_binding = task.lock().unwrap().file.clone();
    let file = String::from(file_binding.to_str().unwrap());
    match task.lock().unwrap().proc_type {
        ProcType::Media => {
            match looper {
                Autoloop::Yes => {
                    thread::spawn(move || {
                        let child = Command::new("ffplay")
                            .arg("-fs")
                            .arg("-loop")
                            .arg("-1")
                            .arg(&file)
                            .spawn().expect("no child");

                        let running_task = RunningTask::new(child, false);
                        task_list_clone.lock().unwrap().push(running_task);
                    });

                }
                Autoloop::No => {
                    thread::spawn(move || {
                        let child = Command::new("ffplay")
                            .arg("-fs")
                            .arg(&file)
                            .spawn().expect("no child");

                        println!("{:?}", child);
                        let running_task = RunningTask::new(child, false);
                        task_list_clone.lock().unwrap().push(running_task);
                    });
                }
            };
        },
        ProcType::Browser => {
            thread::spawn(move || {
                let child = Command::new("chromium")
                    //.arg("--user-data-dir=/tmp/chromium/")
                    //.arg("--disable-session-crashed-bubble")
                    .arg("--disable-infobars")
                    //.arg("--kiosk")
                    .arg("--incognito")
                    .arg("--start-fullscreen")
                    .arg("--start-maximized")
                    .arg(&file)
                    .spawn().expect("no child");

                        let running_task = RunningTask::new(child, false);
                    task_list_clone.lock().unwrap().push(running_task);
            });

        },
        ProcType::Executable => {
            thread::spawn(move || {
                let child = Command::new("sh")
                    .arg(&file)
                    .spawn().expect("no child");

                        let running_task = RunningTask::new(child, false);
                    task_list_clone.lock().unwrap().push(running_task);
            });

        }
    }
}


fn stop_task(task_list: Arc<Mutex<Vec<RunningTask>>>) {
        if task_list.lock().unwrap().len() > 0 {
            let mut task = task_list.lock().unwrap().pop().unwrap();
            task.child.kill().expect("command could not be killed");

            // only one task is run at a time, so it is safe to pop.
            if task.background == false {
                // run background
                background::run(Arc::clone(&task_list));
            }
        }
}

struct App {
    task_list: Arc<Mutex<Vec<RunningTask>>>,
}

impl Default for App {
    fn default() -> Self {
        App {
            task_list: Arc::new(Mutex::new(Vec::new()))
        }
    }
}



fn main() -> Result<(), Box<dyn Error>> {
    
    // initialise the app
    let app = App::default();

    // this will mount all of the drives automatically using udisksctl
    let _mount_drives = identify_mounted_drives();

    let username = whoami::username();
    let env_dir_path: PathBuf =["/home/", &username, ".medialoop_config/vars"].iter().collect();

    if let Err(e) = dotenvy::from_path_override(env_dir_path.as_path()) {
        eprintln!("Cannot find env vars at path: {}", env_dir_path.display());
        eprintln!("Please run medialoop, to setup this program: {}", e);
        process::exit(1)
    }

    let mut file = PathBuf::new();
    let mut proc_type = String::with_capacity(10);
    let mut auto_loop = Autoloop::No;
    // TODO should be updated to AdvancedSchedule enum
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

    // check task elements here
    // does the file exist? 
    if false == file.as_path().exists() {
        display_error_with_message("Could not find file!");    
    }

    let task: Arc<Mutex<Task>> = Arc::new(Mutex::new(Task::new(proc_type, auto_loop, timings, file)));
    
    // create then start the background after the task is created

    
    // set up scheduler
    let mut scheduler = Scheduler::new();
    if schedule == true {
        let _create_background = background::make();
        let _run_background = background::run(Arc::clone(&app.task_list));
       // use the full scheduler and run the task at certain times
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
               let task_clone_2 = Arc::clone(&task);
               let task_list_clone = Arc::clone(&app.task_list);
               let task_list_clone_2 = Arc::clone(&app.task_list);
                // check if day is today 
               let local = Local::now();
               let day = format!("{}", local.format("%d"));
               let is_today = false;  
               let timing_day = timing_vec.as_str();
               if day.lowercase() == timing_day.lowercase() {
                   let start_time =  
                   // if &timing.0 is less 
                   if current_time > &timing.0 && current_time < &timing.1 {
                        run_task(task_list_clone_3.clone(), task_clone_2.clone());
                   }
               }

               scheduler.every(day_name)
                   .at(&timing.0)
                   .run(move || run_task(task_list_clone.clone(), task_clone.clone()));

                scheduler.every(day_name)
                    .at(&timing.1)
                    .run(move || stop_task(task_list_clone_2.clone()));
            }
       }
       loop {
           scheduler.run_pending();
           thread::sleep(Duration::from_millis(10));
       }
   } else {
       // run the task now
       let task_clone = Arc::clone(&task); 
       let task_list_clone = Arc::clone(&app.task_list);
       let _task_aut = run_task(task_list_clone, task_clone);
       loop {}
   }
}
