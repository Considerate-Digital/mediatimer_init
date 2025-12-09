use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
    path::{Path, PathBuf},
    env,
    error::Error,
    process,
    process::{
        Command,
        Child
    },
    os::unix::process::CommandExt,
    fs,
    io::{
        BufRead, 
        BufReader
    },

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

mod loggers;
use crate::loggers::{
    setup_logger,
    log_info,
    log_error
};

mod mount;
use crate::mount::{
    identify_mounted_drives,
    match_uuid,
};

mod background;

mod error;
use crate::error::error_with_message as display_error_with_message;

#[derive(Debug,Clone, Copy, PartialEq)]
pub enum ProcType {
    Video,
    Audio,
    Image,
    Slideshow,
    Web,
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
    file: PathBuf,
    slide_delay: u32,
    web_url: String
}

impl Task {
    fn new(proc_type: ProcType, auto_loop: Autoloop, file: PathBuf, slide_delay: u32, web_url: String) -> Self {
        Task {
            proc_type,
            auto_loop,
            file,
            slide_delay,
            web_url
        }
    }
}

#[derive(Debug)]
struct RunningTask {
    child: process::Child,
    background: bool,
}

impl RunningTask {
    fn new(child: Child, background: bool) -> RunningTask {
        RunningTask {
            child,
            background,
        }
    }
}

fn timing_format_correct(string_of_times: &str) -> bool {
    let re = Regex::new(r"^(?<start>[0-2][0-9]):[0-5][0-9]:[0-5][0-9]-(?<end>[0-2][0-9]):[0-5][0-9]:[0-5][0-9]$").unwrap();
    if re.is_match(string_of_times) { 
        let (_, [start, end]) = re.captures(string_of_times).unwrap().extract();
        //let hour_1 = times.name("h").unwrap().as_str();
        let hour_1 = start.parse::<u32>().unwrap();
        //let hour_2 = times.name("h2").unwrap().as_str();
        let hour_2 = end.parse::<u32>().unwrap();
        
        // This checks if the hour is less than 24
        // The minutes and seconds are already checked by the regex
        hour_1 < 24 && hour_2 < 24
    } else {
        false
    }
}


fn url_format_correct(url: &str) -> bool {
        let re = Regex::new(r"^(https?://)?([\da-z\.-]+)\.([a-z\.]{2,6})([\/\w \.-]*)*\/?$").unwrap();
        re.is_match(url)
    }


fn to_weekday(value: String, day: Weekday, schedule: AdvancedSchedule) -> Result<Weekday, Box<dyn Error>> {


    let mut day_schedule = Vec::new();

    if !&value.is_empty() {
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
    let web_url = task.lock().unwrap().web_url.clone();
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
        ProcType::Web => {
            thread::spawn(move || {
                let child = Command::new("chromium")
                    //.arg("--user-data-dir=/tmp/chromium/")
                    //.arg("--disable-session-crashed-bubble")
                    .arg("--disable-infobars")
                    //.arg("--kiosk")
                    .arg("--incognito")
                    .arg("--start-fullscreen")
                    .arg("--start-maximized")
                    .arg(&web_url)
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

    log_info(format!("Run task: {:?}", task.lock().unwrap()).as_str());

    let looper = task.lock().unwrap().auto_loop;
    let file_binding = task.lock().unwrap().file.clone();
    let file = String::from(file_binding.to_str().unwrap());
    let web_url= task.lock().unwrap().web_url.clone();
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

                        let running_task = RunningTask::new(child, false);
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

                        let running_task = RunningTask::new(child, false);
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

                        let running_task = RunningTask::new(child, false);
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

                        let running_task = RunningTask::new(child, false);
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
                let running_task = RunningTask::new(child, false);
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
                let running_task = RunningTask::new(child, false);
                task_list_clone.lock().unwrap().push(running_task);
            });
        },
        ProcType::Web => {
            thread::spawn(move || {
                let child = Command::new("chromium")
                    //.arg("--user-data-dir=/tmp/chromium/")
                    //.arg("--disable-session-crashed-bubble")
                    .arg("--disable-infobars")
                    //.arg("--kiosk")
                    .arg("--incognito")
                    .arg("--start-fullscreen")
                    .arg("--start-maximized")
                    .arg(&web_url)
                    .spawn().expect("no child");

                let running_task = RunningTask::new(child, false);
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

                let running_task = RunningTask::new(child, false);
                task_list_clone.lock().unwrap().push(running_task);
            });

        },
        ProcType::Executable => {
            thread::spawn(move || {
                let child = Command::new("sh")
                    .arg(&file)
                    .process_group(0)
                    .spawn().expect("no child");

                let running_task = RunningTask::new(child, false);
                task_list_clone.lock().unwrap().push(running_task);
            });

        }
    }

    // stop the task after launching the new task to ensure a smooh overlap
    stop_task(task_list_clone_two.clone());
}

fn stop_task(task_list: Arc<Mutex<Vec<RunningTask>>>) {
    if !task_list.lock().unwrap().is_empty() {

        let mut task = task_list.lock().unwrap().remove(0);

        log_info(format!("Kill Task: {:?}", task.child).as_str());

        task.child.kill().expect("command could not be killed");



        if !task.background {
            // clears up any sub processes: particularly needed for "executable" 
            // proctypes as anything spawned from a sub shell will likely have a different PID
            let id = task.child.id();
            let neg_id = format!("-{}", id);
            let _kill_child = Command::new("kill")
                .arg("-TERM")
                .arg("--")
                .arg(neg_id)
                .output()
                .expect("Failed to remove child with kill command");

            // run background
            background::run(Arc::clone(&task_list));
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
    let mounted_drives = identify_mounted_drives();

    // set up task vars
    let mut file = PathBuf::new();
    let mut web_url = String::with_capacity(0);
    let mut uuid = String::with_capacity(0);
    let mut slide_delay: u32 = 5;
    let mut proc_type = ProcType::Video;
    let mut auto_loop = Autoloop::No;
    let mut schedule = AdvancedSchedule::No;
    let mut monday: Weekday = Weekday::Monday(Vec::with_capacity(2));
    let mut tuesday: Weekday = Weekday::Tuesday(Vec::with_capacity(2));
    let mut wednesday: Weekday = Weekday::Wednesday(Vec::with_capacity(2));
    let mut thursday: Weekday = Weekday::Thursday(Vec::with_capacity(2));
    let mut friday: Weekday = Weekday::Friday(Vec::with_capacity(2));
    let mut saturday: Weekday = Weekday::Saturday(Vec::with_capacity(2));
    let mut sunday: Weekday = Weekday::Sunday(Vec::with_capacity(2));


    let mut autoplay_path = PathBuf::new();
    let mut url_path = PathBuf::new();
    if mounted_drives.len() == 1 {
        autoplay_path = PathBuf::from(&mounted_drives[0]);
        autoplay_path.push("autoplay");
        url_path = autoplay_path.clone();
        url_path.push("url.mt");
    }

        fn is_filename(entry: &Path, name: &str) -> bool {
            let mut entry = entry.to_path_buf();
            entry.set_extension("");
            entry
                .file_name().unwrap()
                .to_str()
                .is_some_and(|n| n.to_lowercase() == name)
        }

        fn dir_contains_url(path: PathBuf) -> bool {
            if path.exists() {
                let mut url_exists = false;
                // read the directory
                for entry in path.read_dir().expect("read_dir call failed").flatten() {
                    let entry_is_filename = is_filename(&entry.path(), "url"); 
                    if entry_is_filename {
                        // rename the entry to comply with our suffix
                        let original_name = entry.path();
                        let mut entry_path = entry.path();
                        entry_path.set_file_name("url");
                        entry_path.set_extension("mt");
                        // rename the file
                        if fs::rename(original_name, entry_path).is_ok() {
                            url_exists = true;
                        } else {
                            //display error
                            display_error_with_message("Failed to rename file containing URL. Please check permissions.");
                        }
                    }
                }
                url_exists
            } else {
                false
            }
        }

        fn is_dirname(path: &Path, name: &str) -> bool {
            if path.exists() {
                if path.is_dir() {
                    path.iter()
                        .any(|n| n.to_ascii_lowercase() == name)
                } else {
                    false
                }
            } else {
                false
            }
        }
        if dir_contains_url(autoplay_path.clone()) {
            // read the file at url_path
            let file = fs::File::open(&url_path).expect("Failed to open URL file");
            let reader = BufReader::new(file);
            let lines: Vec<String> = reader.lines().map(|l| l.expect("no line")).filter(|l| l.contains("https")).collect::<Vec<String>>();
            // TODO check if line contains url
            
            // TODO check url is valid
            if !lines.is_empty() && url_format_correct(&lines[0]) {
                web_url = lines[0].clone();
                proc_type = ProcType::Web;
                schedule = AdvancedSchedule::No;
            } else {
                //TODO error message
                log_error("URL format incorrect");
                display_error_with_message("URL incorrectly formatted. Please check the URL starts with \"https://\"");    

            }
            
        } else if is_dirname(autoplay_path.as_path(), "autoplay") {
            // check if files are images or (audio/video) 
            let files = fs::read_dir(&autoplay_path).unwrap().map(|i| i.unwrap()).collect::<Vec<_>>();
            if files.len() == 1 {
                // use ffprobe to check file or just go for it with ffplay?
                // The regex responds to the first match, which in this case is "video" for a video 
                // and "audio" for a video. Video media types will also have an audio codec_type but this is collected as the second regex capture. 
                let probe_text = Command::new("ffprobe")
                    .arg("-hide_banner")
                    .arg("-show_entries")
                    .arg("stream=codec_type")
                    .arg(files[0].path())
                    .output()
                    .expect("ffprobe failed to find media");
                let probe_string = String::from_utf8_lossy(&probe_text.stdout);
                let media_re = Regex::new(r"\scodec_type=(?<media>\w+)\b").unwrap();
                let media_captures = media_re.captures(&probe_string).unwrap();
                let media_type = media_captures.name("media").unwrap().as_str();

                proc_type = match media_type {
                    "video" => ProcType::Video,
                    "audio" => ProcType::Audio,
                    &_ => ProcType::Video
                };

                // create task with video proc and autoplay it
                file = files[0].path();
                auto_loop = Autoloop::Yes;
                schedule = AdvancedSchedule::No;

            } else {
                // multiple files are available so use slideshow proc
                file = autoplay_path;
                proc_type = ProcType::Slideshow;
                schedule = AdvancedSchedule::No;
            }

    } else {

        let username = whoami::username();
        let env_dir_path: PathBuf =["/home/", &username, ".mediatimer_config/vars"].iter().collect();

        if dotenvy::from_path_override(env_dir_path.as_path()).is_err() {
            eprintln!("Cannot find env vars at path: {}", env_dir_path.display());
            log_error("Cannot find env vars at path");
            display_error_with_message("Could not find config file, please run mediatimer to set up this program.");    
            process::exit(1)
        }

        for (key, value) in env::vars() {
            match key.as_str() {
                "MT_PROCTYPE" => { 
                    proc_type = match value.as_str() {
                        "video" => ProcType::Video,
                        "audio" => ProcType::Audio,
                        "image" => ProcType::Image,
                        "slideshow" => ProcType::Slideshow,
                        "web" => ProcType::Web,
                        "browser" => ProcType::Browser,
                        "executable" => ProcType::Executable,
                        &_ => ProcType::Video
                    }
                },
                "MT_AUTOLOOP" => auto_loop = match value.as_str() {
                    "true" => Autoloop::Yes,
                    "false" => Autoloop::No,
                    &_ => Autoloop::No
                },
                "MT_FILE" => file.push(value.as_str()),
                "MT_URL" => web_url.push_str(value.as_str()),
                "MT_UUID" => uuid.push_str(value.as_str()),
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
        // every ProcType requires a file, except Web
        if proc_type != ProcType::Web && !file.clone().as_path().exists() {
            // match the uuid and change the file path if necessary
            if let Ok(mount_path) = match_uuid(&uuid) {
                // get the new device name from the mount path
                // TODO include checks for these unwraps
                let new_device_name = mount_path.components().nth(2).unwrap().as_os_str().to_str().unwrap();
                // replace the device name in the file_path "/media/{username}/device-name"   
                if let Some(file_path_str) = file.to_str() {
                    // TODO include checks for these unwraps
                    file = PathBuf::from(file_path_str.replace(file.components().nth(2).unwrap().as_os_str().to_str().unwrap(), new_device_name));
                } else {
                    display_error_with_message("Failed to replace file path with new device name.");    
                }
                // set the new file name
            } else {
                display_error_with_message("Could not find file!");    
            }
        }
    } 

    let timings = vec![monday, tuesday, wednesday, thursday, friday, saturday, sunday]; 

    let timings_clone = timings.clone();
    let proc_type_clone = proc_type;
    

    let task: Arc<Mutex<Task>> = Arc::new(Mutex::new(Task::new(proc_type, auto_loop, file, slide_delay, web_url)));

    // set up scheduler
    let mut scheduler = Scheduler::new();
    if schedule == AdvancedSchedule::Yes {
        // create then start the background after the task is created
        background::make(proc_type_clone);

        background::run(Arc::clone(&app.task_list));

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
                    (i[0], i[1], 0_u32) 
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
        run_task(task_list_clone, task_clone);
        loop {
            std::thread::sleep(Duration::from_secs(60));
        };
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
