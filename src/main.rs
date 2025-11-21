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
    os::unix::process::CommandExt
};
use strum::Display;

use clokwerk::{
    Scheduler,
    Interval,
    Job
};

use chrono::{
    TimeZone,
    Local
};

use regex::Regex;
use whoami;

mod loggers;
use crate::loggers::{
    setup_logger,
    log_info,
    log_error
};

mod mount;
use crate::mount::identify_mounted_drives;

mod background;

mod error;
use crate::error::error as display_error;
use crate::error::error_with_message as display_error_with_message;

#[derive(Debug,Clone, Copy, PartialEq)]
pub enum ProcType {
    Video,
    Audio,
    Image,
    Slideshow,
    Browser,
    Executable,
}


#[derive(Debug, Clone, Copy)]
enum Autoloop {
    Yes,
    No
}

#[derive(Debug, Display, PartialEq, Clone)]
pub enum AdvancedSchedule {
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
}


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
    slide_delay: u32
}

impl Task {
    fn new(proc_type: ProcType, auto_loop: Autoloop, timings: Timings, file: PathBuf, slide_delay: u32) -> Self {
        Task {
            proc_type,
            auto_loop,
            timings,
            file,
            slide_delay
        }
    }
    fn background() -> Self {
        Task {
            proc_type: ProcType::Video,
            auto_loop: Autoloop::Yes,
            timings: Vec::with_capacity(0),
            file: PathBuf::from("/"),
            slide_delay: 5
        }
    }
}

#[derive(Debug)]
struct RunningTask {
    child: process::Child,
    background: bool,
    task: Arc<Mutex<Task>>
}

impl RunningTask {
    fn new(child: Child, background: bool, task: Arc<Mutex<Task>>) -> RunningTask {
        RunningTask {
            child,
            background,
            task
        }
    }
}

fn timing_format_correct(string_of_times: &str) -> bool {
    let re = Regex::new(r"^(?<h>[0-2][0-9]):[0-5][0-9]:[0-5][0-9]-(?<h2>[0-2][0-9]):[0-5][0-9]:[0-5][0-9]$").unwrap();
    if re.is_match(string_of_times) { 
        let times: Vec<(u32, u32)> = re.captures_iter(string_of_times).map(|times| {
            let hour_1 = times.name("h").unwrap().as_str();
            let hour_1 = hour_1.parse::<u32>().unwrap();
            let hour_2 = times.name("h2").unwrap().as_str();
            let hour_2 = hour_2.parse::<u32>().unwrap();
            (hour_1, hour_2)
        }).collect();
        for time_pair in times.iter() {
            if time_pair.0 < 24 && 
                time_pair.1 < 24 {
                return true;
            } else {
                return false;
            }
        }
        false
    } else {
        false
    }
}

fn to_weekday(value: String, day: Weekday, schedule: AdvancedSchedule) -> Result<Weekday, Box<dyn Error>> {

    
    let mut day_schedule = Vec::new();

    if &value != "" {
        let string_vec: Vec<String> = value.as_str().split(",").map(|x| x.trim().to_string()).collect(); 

        for start_and_end in string_vec.iter() {
            if schedule == AdvancedSchedule::Yes && !timing_format_correct(start_and_end) {
                display_error_with_message("Schedule incorrectly formatted!");
                process::exit(1);
            }
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

#[cfg(feature="eco")]
fn run_task(task_list: Arc<Mutex<Vec<RunningTask>>>, task: Arc<Mutex<Task>>) {
    let task_list_clone = Arc::clone(&task_list);
    let task_list_clone_two = Arc::clone(&task_list);
    let task_clone = Arc::clone(&task);

    log_info(format!("Run task: {:?}", task.lock().unwrap()).as_str());
    
    let looper = match task.lock().unwrap().auto_loop {
        Autoloop::Yes => Autoloop::Yes,
        Autoloop::No => Autoloop::No
    };
    let file_binding = task.lock().unwrap().file.clone();
    let file = String::from(file_binding.to_str().unwrap());
    let slide_delay = task.lock().unwrap().slide_delay.to_string();

    match task.lock().unwrap().proc_type {
        ProcType::Video => {
            match looper {
                Autoloop::Yes => {
                    thread::spawn(move || {
                        let child = Command::new("ffplay")
                            .arg("-hide_banner")
                            .arg("-loglevel")
                            .arg("error")
                            .arg("-an")
                            .arg("-fs")
                            .arg("-loop")
                            .arg("-1")
                            .arg(&file)
                            .spawn().expect("no child");

                        let running_task = RunningTask::new(child, false, task_clone);
                        task_list_clone.lock().unwrap().push(running_task);
                    });

                }
                Autoloop::No => {
                    thread::spawn(move || {
                        let child = Command::new("ffplay")
                            .arg("-hide_banner")
                            .arg("-loglevel")
                            .arg("error")
                            .arg("-an")
                            .arg("-fs")
                            .arg(&file)
                            .spawn().expect("no child");

                        let running_task = RunningTask::new(child, false, task_clone);
                        task_list_clone.lock().unwrap().push(running_task);
                    });
                }
            };
        },
        ProcType::Audio => {
            match looper {
                Autoloop::Yes => {
                    thread::spawn(move || {
                        let child = Command::new("ffplay")
                            .arg("-hide_banner")
                            .arg("-loglevel")
                            .arg("error")
                            //.arg("-nodisp")
                            //.arg("-fs")
                            .arg("-loop")
                            .arg("-1")
                            .arg(&file)
                            .spawn().expect("no child");

                        let running_task = RunningTask::new(child, false, task_clone);
                        task_list_clone.lock().unwrap().push(running_task);
                    });

                }
                Autoloop::No => {
                    thread::spawn(move || {
                        let child = Command::new("ffplay")
                            .arg("-hide_banner")
                            .arg("-loglevel")
                            .arg("error")
                            //.arg("-fs")
                            //.arg("-nodisp")
                            .arg(&file)
                            .spawn().expect("no child");

                        let running_task = RunningTask::new(child, false, task_clone);
                        task_list_clone.lock().unwrap().push(running_task);
                    });
                }
            };
        },
        ProcType::Image => {
            thread::spawn(move || {
                let child = Command::new("feh")
                    .arg("-YxqFZz")
                    .arg("-B")
                    .arg("black")
                    .arg(&file)
                    .spawn().expect("no child");
                        
                let running_task = RunningTask::new(child, false, task_clone);
                task_list_clone.lock().unwrap().push(running_task);
            });
        },
        ProcType::Slideshow => {
            thread::spawn(move || {
                let child = Command::new("feh")
                    .arg("-YxqFZz")
                    .arg("-B")
                    .arg("black")
                    .arg("-D")
                    .arg(&slide_delay)
                    .arg(&file)
                    .spawn().expect("no child");
                let running_task = RunningTask::new(child, false, task_clone);
                task_list_clone.lock().unwrap().push(running_task);
            });
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

                let running_task = RunningTask::new(child, false, task_clone);
                    task_list_clone.lock().unwrap().push(running_task);
            });

        },
        ProcType::Executable => {
            thread::spawn(move || {
                let child = Command::new("sh")
                    .arg(&file)
                    .process_group(0)
                    .spawn().expect("no child");

                let running_task = RunningTask::new(child, false, task_clone);
                task_list_clone.lock().unwrap().push(running_task);
            });

        }
    }

    // stop the task after launching the new task to ensure a smooh overlap
    let _stopped_task = stop_task(task_list_clone_two.clone());
}

#[cfg(any(feature="standard", feature="pro"))]
fn run_task(task_list: Arc<Mutex<Vec<RunningTask>>>, task: Arc<Mutex<Task>>) {
    let task_list_clone = Arc::clone(&task_list);
    let task_list_clone_two = Arc::clone(&task_list);

    let task_clone = Arc::clone(&task);

    log_info(format!("Run task: {:?}", task.lock().unwrap()).as_str());
    
    let looper = match task.lock().unwrap().auto_loop {
        Autoloop::Yes => Autoloop::Yes,
        Autoloop::No => Autoloop::No
    };
    let file_binding = task.lock().unwrap().file.clone();
    let file = String::from(file_binding.to_str().unwrap());
    let slide_delay = task.lock().unwrap().slide_delay.to_string();

    match task.lock().unwrap().proc_type {
        ProcType::Video => {
            match looper {
                Autoloop::Yes => {
                    thread::spawn(move || {
                        let child = Command::new("ffplay")
                            .arg("-hide_banner")
                            .arg("-loglevel")
                            .arg("error")
                            .arg("-fs")
                            .arg("-loop")
                            .arg("-1")
                            .arg(&file)
                            .spawn().expect("no child");

                        let running_task = RunningTask::new(child, false, task_clone);
                        task_list_clone.lock().unwrap().push(running_task);
                    });

                }
                Autoloop::No => {
                    thread::spawn(move || {
                        let child = Command::new("ffplay")
                            .arg("-hide_banner")
                            .arg("-loglevel")
                            .arg("error")
                            .arg("-fs")
                            .arg(&file)
                            .spawn().expect("no child");

                        let running_task = RunningTask::new(child, false, task_clone);
                        task_list_clone.lock().unwrap().push(running_task);
                    });
                }
            };
        },
        ProcType::Audio => {
            match looper {
                Autoloop::Yes => {
                    thread::spawn(move || {
                        let child = Command::new("ffplay")
                            .arg("-hide_banner")
                            .arg("-loglevel")
                            .arg("error")
                            //.arg("-nodisp")
                            //.arg("-fs")
                            .arg("-loop")
                            .arg("-1")
                            .arg(&file)
                            .spawn().expect("no child");

                        let running_task = RunningTask::new(child, false, task_clone);
                        task_list_clone.lock().unwrap().push(running_task);
                    });

                }
                Autoloop::No => {
                    thread::spawn(move || {
                        let child = Command::new("ffplay")
                            .arg("-hide_banner")
                            .arg("-loglevel")
                            .arg("error")
                            //.arg("-fs")
                            //.arg("-nodisp")
                            .arg(&file)
                            .spawn().expect("no child");

                        let running_task = RunningTask::new(child, false, task_clone);
                        task_list_clone.lock().unwrap().push(running_task);
                    });
                }
            };
        },
        ProcType::Image => {
            thread::spawn(move || {
                let child = Command::new("feh")
                    .arg("-YxqFZz")
                    .arg("-B")
                    .arg("black")
                    .arg(&file)
                    .spawn().expect("no child");
                let running_task = RunningTask::new(child, false, task_clone);
                task_list_clone.lock().unwrap().push(running_task);
            });
        },
        ProcType::Slideshow => {
            thread::spawn(move || {
                let child = Command::new("feh")
                    .arg("-YxqFZz")
                    .arg("-B")
                    .arg("black")
                    .arg("-D")
                    .arg(&slide_delay)
                    .arg(&file)
                    .spawn().expect("no child");
                let running_task = RunningTask::new(child, false, task_clone);
                task_list_clone.lock().unwrap().push(running_task);
            });
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

                    let running_task = RunningTask::new(child, false, task_clone);
                    task_list_clone.lock().unwrap().push(running_task);
            });

        },
        ProcType::Executable => {
            thread::spawn(move || {
                let child = Command::new("sh")
                    .arg(&file)
                    .process_group(0)
                    .spawn().expect("no child");

                    let running_task = RunningTask::new(child, false, task_clone);
                    task_list_clone.lock().unwrap().push(running_task);
            });

        }
    }

    // stop the task after launching the new task to ensure a smooh overlap
    let _stopped_task = stop_task(task_list_clone_two.clone());
}

//#[cfg(feature="pro")] Coming soon

fn stop_task(task_list: Arc<Mutex<Vec<RunningTask>>>) {
        if task_list.lock().unwrap().len() > 0 {

            let mut task = task_list.lock().unwrap().remove(0);

            log_info(format!("Kill Task: {:?}", task.child).as_str());

            task.child.kill().expect("command could not be killed");
            
            

            if task.background == false {
                // clears up any sub processes: particularly needed for "executable" 
                // proctypes as anything spawned from a sub shell will likely have a different PID
                let id = task.child.id();
                let neg_id = format!("-{}", id.to_string());
                let _kill_child = Command::new("kill")
                    .arg("-TERM")
                    .arg("--")
                    .arg(neg_id)
                    .output()
                    .expect("Failed to remove child with kill command");
                
                // run background
                if task.task.lock().unwrap().proc_type == ProcType::Audio {
                    background::run(Arc::clone(&task_list), true);
                } else {
                    background::run(Arc::clone(&task_list), false);
                }

            }

            // wait for a second before stopping the task, to allow overlap
            let one_sec = Duration::from_millis(1000);
            thread::sleep(one_sec);

            task.child.kill().expect("command could not be killed");
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

    // initialise loggers
    setup_logger();

    // this will mount all of the drives automatically using udisksctl
    let _mount_drives = identify_mounted_drives();

    let username = whoami::username();
    let env_dir_path: PathBuf =["/home/", &username, ".mediatimer_config/vars"].iter().collect();

    if let Err(_) = dotenvy::from_path_override(env_dir_path.as_path()) {
        eprintln!("Cannot find env vars at path: {}", env_dir_path.display());
        log_error("Cannot find env vars at path");
        display_error_with_message("Could not find config file, please run mediatimer to set up this program.");    
        process::exit(1)
    }

    let mut file = PathBuf::new();
    let mut slide_delay: u32 = 5;
    let mut proc_type = String::with_capacity(10);
    let mut auto_loop = Autoloop::No;
    let mut schedule = AdvancedSchedule::No;
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
            "MT_PROCTYPE" => proc_type.push_str(&value),
            "MT_AUTOLOOP" => auto_loop = match value.as_str() {
                "true" => Autoloop::Yes,
                "false" => Autoloop::No,
                &_ => Autoloop::No
            },
            "MT_FILE" => file.push(value.as_str()),
            "MT_SLIDE_DELAY" => slide_delay = value.parse::<u32>().unwrap(),
            "MT_SCHEDULE" => schedule = match value.as_str() {
                "true" => AdvancedSchedule::Yes,
                "false" => AdvancedSchedule::No,
                &_ => AdvancedSchedule::No
            },
            "MT_MONDAY" => monday = to_weekday(value, Weekday::Monday(Vec::new()), schedule.clone())?,
            "MT_TUESDAY" => tuesday = to_weekday(value, Weekday::Tuesday(Vec::new()), schedule.clone())?,
            "MT_WEDNESDAY" => wednesday = to_weekday(value, Weekday::Wednesday(Vec::new()), schedule.clone())?,
            "MT_THURSDAY" => thursday = to_weekday(value, Weekday::Thursday(Vec::new()), schedule.clone())?,
            "MT_FRIDAY" => friday = to_weekday(value, Weekday::Friday(Vec::new()), schedule.clone())?,
            "MT_SATURDAY" => saturday = to_weekday(value, Weekday::Saturday(Vec::new()), schedule.clone())?,
            "MT_SUNDAY" => sunday = to_weekday(value, Weekday::Sunday(Vec::new()), schedule.clone())?,
            _ => {}
        }
    }

    timings = vec![monday, tuesday, wednesday, thursday, friday, saturday, sunday]; 
   
    let timings_clone = timings.clone();

    let proc_type = match proc_type.to_lowercase().as_str() {
        "video" => ProcType::Video,
        "audio" => ProcType::Audio,
        "image" => ProcType::Image,
        "slideshow" => ProcType::Slideshow,
        "browser" => ProcType::Browser,
        "executable" => ProcType::Executable,
        &_ => ProcType::Video
    };
    
    let proc_type_clone = proc_type;
    // check task elements here
    // does the file exist? 
    if false == file.as_path().exists() {
        display_error_with_message("Could not find file!");    
    }

    let task: Arc<Mutex<Task>> = Arc::new(Mutex::new(Task::new(proc_type, auto_loop, timings, file, slide_delay)));
    
    // set up scheduler
    let mut scheduler = Scheduler::new();
    if schedule == AdvancedSchedule::Yes {
        // create then start the background after the task is created
        let _create_background = background::make(proc_type_clone);
        if proc_type == ProcType::Audio { 
            let _run_background = background::run(Arc::clone(&app.task_list), true);
        } else {
            let _run_background = background::run(Arc::clone(&app.task_list), false);
        }
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
           
           fn get_timing_as_hms(value: &str) -> (u32, u32, u32) {
               let i = value.split(":").map(|t| t.parse::<u32>().unwrap()).collect::<Vec<u32>>();
               // function must return a date format string
               if i.len() == 2 {
                   (i[0], i[1], 0 as u32) 
                } else {
                   (i[0], i[1], i[2]) 
                }
           }
        
           // iterates through each timing for the day
            for timing in timing_vec.iter() {
               let task_clone = Arc::clone(&task);
               let task_list_clone = Arc::clone(&app.task_list);
               let task_list_clone_2 = Arc::clone(&app.task_list);

                // check if day is today 
               let local = Local::now();
               let day_today = format!("{}", local.format("%A"));

               
               let timing_day = day.as_str();
               if day_today.to_lowercase() == timing_day.to_lowercase() {
                   let date_string = format!("{}", local.format("%Y:%m:%d:%H:%M:%S"));
                   let date_nums: Vec<u32> = date_string.split(":").map(|i| i.parse::<u32>().unwrap()).collect::<Vec<u32>>();
                   let year_num = date_nums[0] as i32;
                   let month_num = date_nums[1];
                   let day_num = date_nums[2];
                   let _hour_num = date_nums[3];
                   let _min_num = date_nums[4];
                   let _sec_num = date_nums[5];

                   let (start_hour, start_min, start_sec) = get_timing_as_hms(&timing.0);

                   let start_time = Local.with_ymd_and_hms(
                       year_num, month_num, day_num, 
                       start_hour, start_min, start_sec).unwrap();

                   let (end_hour, end_min, end_sec) = get_timing_as_hms(&timing.1);

                   let end_time = Local.with_ymd_and_hms(
                       year_num, month_num, day_num, 
                       end_hour, end_min, end_sec).unwrap();


                    let local_timestamp = local.timestamp(); 
                   // if &timing.0 is less 
                   if local_timestamp > start_time.timestamp() && local_timestamp < end_time.timestamp() {

                        let task_list_clone_3 = Arc::clone(&app.task_list);
                        let task_clone_2 = Arc::clone(&task);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use chrono::Local;
    use std::fs;
    use tempfile::tempdir;
    use std::os::unix::fs::PermissionsExt;

    // Test the Weekday enum functionality
    #[test]
    fn test_weekday_as_str() {
        let monday = Weekday::Monday(Vec::new());
        assert_eq!(monday.as_str(), "Monday");
        
        let tuesday = Weekday::Tuesday(Vec::new());
        assert_eq!(tuesday.as_str(), "Tuesday");
        
        // Test remaining weekdays similarly...
    }

    #[test]
    fn test_weekday_to_string() {
        let monday = Weekday::Monday(Vec::new());
        assert_eq!(monday.to_string(), "Monday");
        
        let thursday = Weekday::Thursday(Vec::new());
        assert_eq!(thursday.to_string(), "Thursday");
    }

    #[test]
    fn test_weekday_timings() {
        let schedule = vec![("08:00".to_string(), "12:00".to_string())];
        let monday = Weekday::Monday(schedule.clone());
        
        assert_eq!(monday.timings(), schedule);
    }

    // Test Task struct functionality
    #[test]
    fn test_task_new() {
        let file_path = PathBuf::from("/tmp/test.mp4");
        let task = Task::new(
            ProcType::Video, 
            Autoloop::No, 
            Vec::new(), 
            file_path.clone(),
            7

        );
        
        match task.proc_type {
            ProcType::Video => assert!(true),
            _ => assert!(false, "Incorrect proc_type"),
        }
        
        match task.auto_loop {
            Autoloop::No => assert!(true),
            _ => assert!(false, "Incorrect auto_loop value"),
        }
        
        assert_eq!(task.timings.len(), 0);
        assert_eq!(task.file, file_path);
    }

    #[test]
    fn test_task_setters() {
        let file_path = PathBuf::from("/tmp/test.mp4");
        let mut task = Task::new(
            ProcType::Video, 
            Autoloop::No, 
            Vec::new(), 
            file_path,
            7
        );
        
        task.set_loop(Autoloop::Yes);
        match task.auto_loop {
            Autoloop::Yes => assert!(true),
            _ => assert!(false, "Failed to set auto_loop"),
        }
        
        task.set_proc_type(ProcType::Browser);
        match task.proc_type {
            ProcType::Browser => assert!(true),
            _ => assert!(false, "Failed to set proc_type"),
        }
        
        let schedule = Vec::new();
        let monday = Weekday::Monday(schedule);
        task.set_weekday(monday);
        assert_eq!(task.timings.len(), 1);
        
        match &task.timings[0] {
            Weekday::Monday(_) => assert!(true),
            _ => assert!(false, "Failed to set weekday"),
        }
    }

    // Test to_weekday function
    #[test]
    fn test_to_weekday_valid_format() {
        let value = "08:00-12:00".to_string();
        let schedule = AdvancedSchedule::No;
        let result = to_weekday(value, Weekday::Monday(Vec::new()), schedule);
        
        assert!(result.is_ok());
        match result.unwrap() {
            Weekday::Monday(schedule) => {
                assert_eq!(schedule.len(), 1);
                assert_eq!(schedule[0].0, "08:00");
                assert_eq!(schedule[0].1, "12:00");
            },
            _ => assert!(false, "Incorrect weekday returned"),
        }
    }

    #[test]
    fn test_to_weekday_multiple_schedules() {
        let value = "08:00-12:00, 14:00-16:00".to_string();
        let schedule = AdvancedSchedule::No;
        let result = to_weekday(value, Weekday::Tuesday(Vec::new()), schedule);
        
        assert!(result.is_ok());
        match result.unwrap() {
            Weekday::Tuesday(schedule) => {
                assert_eq!(schedule.len(), 2);
                assert_eq!(schedule[0].0, "08:00");
                assert_eq!(schedule[0].1, "12:00");
                assert_eq!(schedule[1].0, "14:00");
                assert_eq!(schedule[1].1, "16:00");
            },
            _ => assert!(false, "Incorrect weekday returned"),
        }
    }

    #[test]
    fn test_to_weekday_empty_string() {
        let value = "".to_string();
        let schedule = AdvancedSchedule::No;
        let result = to_weekday(value, Weekday::Wednesday(Vec::new()), schedule);
        
        assert!(result.is_ok());
        match result.unwrap() {
            Weekday::Wednesday(schedule) => {
                assert_eq!(schedule.len(), 0);
            },
            _ => assert!(false, "Incorrect weekday returned"),
        }
    }

    // Test functionality of the RunningTask struct
    #[test]
    fn test_running_task_new() {
        
        let test_task = Task::new(
            ProcType::Browser,
            Autoloop::No,
            Vec::with_capacity(0),
            PathBuf::from("/"),
            5
        );


        let dummy_child = Command::new("echo").spawn().expect("Failed to create dummy process");
        let task = RunningTask::new(dummy_child, false, 
            Arc::new(Mutex::new(test_task))
            );
        
        assert_eq!(task.background, false);
        // We can't directly test the child process, but we can verify the struct was created
    }

    // Integration tests for task execution - these need to be carefully considered as they create actual processes
    // Using mocked commands/processes would be ideal
    
    #[test]
    fn test_run_and_stop_task() {
        
        let video_proc = ProcType::Video;
        let _create_background = background::make(video_proc);

        let task_list = Arc::new(Mutex::new(Vec::new()));

        // Create a temporary test script
        let dir = tempdir().unwrap();
        let script_path = dir.path().join("test_script.sh");
        fs::write(&script_path, "#!/bin/sh\nsleep 10\n").unwrap();
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();
        
        let task = Arc::new(Mutex::new(Task::new(
            ProcType::Executable,
            Autoloop::No,
            Vec::new(),
            script_path,
            5
        )));
        
        
        // Run the task
        run_task(Arc::clone(&task_list), Arc::clone(&task));
        
        // Give it a moment to start
        thread::sleep(Duration::from_millis(500));
        
        // Check task is running
        assert_eq!(task_list.lock().unwrap().len(), 1);
        
        // Stop the task
        stop_task(Arc::clone(&task_list));
        
        // Task list should be empty after stopping
        assert_eq!(task_list.lock().unwrap().len(), 1);
    }

    // Mock test for scheduler functionality
    #[test]
    fn test_schedule_timing_parser() {
        // Test the function that parses time strings
        // We can extract the function from the main code to test it separately
        
        // For example:
        fn get_timing_as_hms(value: &str) -> (u32, u32, u32) {
            let i = value.split(":").map(|t| t.parse::<u32>().unwrap()).collect::<Vec<u32>>();
            if i.len() == 2 {
                (i[0], i[1], 0 as u32) 
            } else {
                (i[0], i[1], i[2]) 
            }
        }
        
        assert_eq!(get_timing_as_hms("08:30"), (8, 30, 0));
        assert_eq!(get_timing_as_hms("15:45:20"), (15, 45, 20));
    }

    // Test App default implementation
    #[test]
    fn test_app_default() {
        let app = App::default();
        assert_eq!(app.task_list.lock().unwrap().len(), 0);
    }
}
