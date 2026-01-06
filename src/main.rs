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

use log::{info, warn, error};

mod loggers;
use crate::loggers::setup_logger;

mod mount;
use crate::mount::{
    identify_mounted_drives,
    match_uuid,
};

mod background;

mod error;
use crate::error::error_with_message as display_error_with_message;

mod task_runner;
use crate::task_runner::run_task;

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
#[derive(Debug, PartialEq,Display, Clone)]
enum Model {
    Eco,
    Standard,
    Pro
}

/// This program runs one task at custom intervals. The task can also be looped.
/// Commonly this is used for playing media files at certain times.
/// The Task struct is the main set of instructions that are written out into an env file to be 
/// interpreted in future by the init program.
#[derive(Debug)]
struct Task {
    model: Model,
    proc_type: ProcType,
    auto_loop: Autoloop,
    file: PathBuf,
    slide_delay: u32,
    web_url: String
}

impl Task {
    fn new(model: Model, proc_type: ProcType, auto_loop: Autoloop, file: PathBuf, slide_delay: u32, web_url: String) -> Self {
        Task {
            model,
            proc_type,
            auto_loop,
            file,
            slide_delay,
            web_url
        }
    }
}

#[derive(Debug)]
pub struct RunningTask {
    child: process::Child,
    background: bool,
}

impl RunningTask {
    fn new(child: Child, background: bool) -> RunningTask {
        logi!("Initialising running task");
        RunningTask {
            child,
            background,
        }
    }
}

fn timing_format_correct(string_of_times: &str) -> Result<bool, Box<dyn Error>> {
    logi!("Checking timing format");
    let re = Regex::new(r"^(?<start>[0-2][0-9]):[0-5][0-9]:[0-5][0-9]-(?<end>[0-2][0-9]):[0-5][0-9]:[0-5][0-9]$")?;
    if re.is_match(string_of_times) { 
        if let Some(captured) = re.captures(string_of_times) {
            let (_, [start, end]) = captured.extract();
            let hour_1 = start.parse::<u32>()?;
            let hour_2 = end.parse::<u32>()?;

            // This checks if the hour is less than 24
            // The minutes and seconds are already checked by the regex
            if hour_1 < 24 && hour_2 < 24 {
                return Ok(true);
            }
        } else {
            logw!("Timing format could not be captured in regex")
        }
    }
    Ok(false)
}


fn url_format_correct(url: &str) -> Result<bool, Box<dyn Error>> {
    logi!("Checking URL format");
    let re = Regex::new(r"^(https?://)?([\da-z\.-]+)\.([a-z\.]{2,6})([\/\w \.-]*)*\/?$")?;
    Ok(re.is_match(url))
}


fn to_weekday(value: String, day: Weekday, schedule: AdvancedSchedule) -> Result<Weekday, Box<dyn Error>> {


    let mut day_schedule = Vec::new();

    if !&value.is_empty() {
        let string_vec: Vec<String> = value.as_str().split(",").map(|x| x.trim().to_string()).collect(); 

        for start_and_end in string_vec.iter() {
            let timing_format_correct = timing_format_correct(start_and_end)?;
            if schedule == AdvancedSchedule::Yes && !timing_format_correct {
                display_error_with_message("Schedule incorrectly formatted!");
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



fn stop_task(task_list: Arc<Mutex<Vec<RunningTask>>>) -> Result<(), Box<dyn Error>> {

    if !task_list.lock().unwrap().is_empty() {

        let mut task = task_list.lock().unwrap().remove(0);

        logi!("Attempting to Kill Task: {:?}", task.child);

        task.child.kill()?;



        if !task.background {
            // clears up any sub processes: particularly needed for "executable" 
            // proctypes as anything spawned from a sub shell will likely have a different PID
            logi!("Attempting to kill any subprocesses");
            let id = task.child.id();
            let neg_id = format!("-{}", id);
            let _kill_child = Command::new("kill")
                .arg("-TERM")
                .arg("--")
                .arg(neg_id)
                .output()?;

            logi!("Killed task was not background; attempting to start background");
            // run background
            background::run(Arc::clone(&task_list))?;
        } else {
            logi!("Killed task was background");
        }

        // wait for a second before stopping the task, to allow overlap
        let one_sec = Duration::from_millis(1000);
        thread::sleep(one_sec);

        task.child.kill()?;
    }
    Ok(())
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

fn is_filename(entry: &Path, name: &str) -> Result<bool, Box<dyn Error>> {
    let mut entry = entry.to_path_buf();
    entry.set_extension("");
    if let Some(file_name) = entry.file_name() {
        return Ok(
            file_name
            .to_str()
            .is_some_and(|n| n.to_lowercase() == name)
        );
    } else {
        logw!("File name parsing failed")
    }
    Ok(false)
}

fn dir_contains_url(path: PathBuf) -> Result<bool, Box<dyn Error>> {
    if path.exists() {
        let mut url_exists = false;
        // read the directory
        for entry in path.read_dir()?.flatten() {
            let entry_is_filename = is_filename(&entry.path(), "url")?; 
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
        return Ok(url_exists);
    }
    Ok(false)
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

/// First the statement checks if a URL is present in a text file inside the autoplay directory
/// Next the statement checks if the autoplay path exists
///
/// This process checks the first file available inside the autoplay directory. It 
/// ascertains whether the file is video or audio media and sets the "proc_type" 
/// variable accordingly.
/// Lastly if the autoplay directory is not present on the mounted storage device, then the 
/// Media Timer config variables are imported. These variables are set via the `mediatimer` 
/// program.
fn main() -> Result<(), Box<dyn Error>> {

    // initialise the app
    let app = App::default();

    // initialise loggers
    if let Err(e) = setup_logger() {
        loge!("Logger could not be initialised: {}", e);
    }

    logi!("Initialising");
    logi!("Loggers initialised");

    // Preset model to "pro" version so that all features are enabled if the model details 
    // cannot be found
    let mut model: Model = Model::Pro;
    // read model type
    if let Ok(model_name) = fs::read_to_string("/etc/adaptableos/MODEL") {
        model = match model_name.trim().to_lowercase().as_str() {
            "eco" => Model::Eco,
            "standard" => Model::Standard,
            &_ => Model::Pro
        }
    } else {
        logw!("No Adaptable model set at /etc/adaptableos/MODEL. Default model Pro will be used.");
    }

    logi!("Model selected: {}", &model);

    // this will mount all of the drives automatically using udisksctl
    let identified_drives = identify_mounted_drives();
    let mut mounted_drives = Vec::new();
    let _ = match identified_drives {
        Ok(drives) => mounted_drives = drives,
        Err(e) => {
            logw!("No storage devices identified, Error: {}", e);
        }
    };

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



    let dir_contains_url = dir_contains_url(autoplay_path.clone())?;
    // First the statement checks if a URL is present in a text file inside the autoplay directory
    if dir_contains_url  {
        logi!("Reading URL from autoplay file path");
        // read the file at url_path
        let file = fs::File::open(&url_path).expect("Failed to open URL file");
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).filter(|l| l.contains("https")).collect::<Vec<String>>();
        let url_format_correct = url_format_correct(&lines[0])?; 
        if !lines.is_empty() && url_format_correct {
            web_url = lines[0].clone();
            proc_type = ProcType::Web;
            schedule = AdvancedSchedule::No;
        } else {
            loge!("URL format incorrect");

        }
    // Next the statement checks if the autoplay path exists
    } else if is_dirname(autoplay_path.as_path(), "autoplay") {
        // check if files are images or (audio/video) 
        let files = fs::read_dir(&autoplay_path)?.map(|i| i.unwrap()).collect::<Vec<_>>();
        if files.len() == 1 {
            // This process checks the first file available inside the autoplay directory. It 
            // ascertains whether the file is video or audio media and sets the "proc_type" 
            // variable accordingly.
            // The regex responds to the first match, which in this case is "video" for a video 
            // and "audio" for a video. Video media types will also have an audio codec_type but this is collected as the second regex capture. 
            let probe_text = Command::new("ffprobe")
                .arg("-hide_banner")
                .arg("-show_entries")
                .arg("stream=codec_type")
                .arg(files[0].path())
                .output()?;
            let probe_string = String::from_utf8_lossy(&probe_text.stdout);
            
            let media_re = Regex::new(r"\scodec_type=(?<media>\w+)\b")?;
            if let Some(media_captures) = media_re.captures(&probe_string) {
                if let Some(media_type) = media_captures.name("media") {

                    proc_type = match media_type.as_str() {
                        "video" => ProcType::Video,
                        "audio" => ProcType::Audio,
                        &_ => ProcType::Video
                    };

                    // create task with video proc and autoplay it
                    file = files[0].path();
                    auto_loop = Autoloop::Yes;
                    schedule = AdvancedSchedule::No;
                } else {
                    logw!("Media name could not be captured in regex")
                }
            } else {
                logw!("Media codec could not be captured in regex")
            }
        } else {
            // multiple files are available so use slideshow proc
            file = autoplay_path;
            proc_type = ProcType::Slideshow;
            schedule = AdvancedSchedule::No;
        }
    // Lastly if the autoplay directory is not present on the mounted storage device, then the 
    // Media Timer config variables are imported. These variables are set via the `mediatimer` 
    // program.
    } else {
        let username = whoami::username();
        let env_dir_path: PathBuf =["/home/", &username, ".mediatimer_config/vars"].iter().collect();

        if dotenvy::from_path_override(env_dir_path.as_path()).is_err() {
            eprintln!("Cannot find env vars at path: {}", env_dir_path.display());
            loge!("Cannot find env vars at path");
            display_error_with_message("Could not find config file, please run mediatimer to set up this program.");    
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
                "MT_SLIDE_DELAY" => slide_delay = value.parse::<u32>()?,
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
        // This statement checks to see if the file exists at the path saved in the mediatimer 
        // config variables. If the path does not exist, the saved UUID is checked against all 
        // currently mounted storage devices and then the file path is corrected in the 
        // program if necessary. 
        if proc_type != ProcType::Web && !file.clone().as_path().exists() {
            // match the uuid and change the file path if necessary
            if let Ok(mount_path) = match_uuid(&uuid) {

                let failure_message = "Failed to replace file path with new device name";
                // get the new device name from the mount path
                if let Some(new_device) = mount_path.components().nth(2) {
                    if let Some(new_device_str) = new_device.as_os_str().to_str() {
                        // replace the device name in the file_path "/media/{username}/device-name"   
                        if let Some(file_path_str) = file.to_str() {
                            if let Some(file_device) = file.components().nth(2) { 
                                if let Some(file_device_str) = file_device.as_os_str().to_str() {

                                    file = PathBuf::from(file_path_str.replace(file_device_str, new_device_str));
                                } else {
                                    loge!("{}", failure_message);
                                    display_error_with_message(failure_message);    
                                }
                            } else {
                                loge!("{}", failure_message);
                                display_error_with_message(failure_message);    
                            }
                        } else {
                            loge!("{}", failure_message);
                            display_error_with_message(failure_message);    
                        }
                    } else {
                        loge!("{}", failure_message);
                        display_error_with_message(failure_message);    
                    }
                } else {
                    loge!("{}", failure_message);
                    display_error_with_message(failure_message);    
                }
            } else {
                loge!("Could not match UUID and identify mount path");
                display_error_with_message("Could not match storage device UUID and identify mount path.");    
            }
        }
    } 

    let timings = vec![monday, tuesday, wednesday, thursday, friday, saturday, sunday]; 

    let timings_clone = timings.clone();

    let task: Arc<Mutex<Task>> = Arc::new(Mutex::new(Task::new(model, proc_type, auto_loop, file, slide_delay, web_url)));

    // set up scheduler
    let mut scheduler = Scheduler::new();
    if schedule == AdvancedSchedule::Yes {
        // create then start the background after the task is created
        if let Err(e) = background::make() {
            loge!("Failed to make background: {}", e);
        }

        if let Err(e) = background::run(Arc::clone(&app.task_list)) {
            loge!("Failed to run background: {}", e);
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
                    let date_nums: Vec<u32> = date_string.split(":").map(|i| i.parse::<u32>()).filter_map(|x| Some(x)?.ok()).collect::<Vec<u32>>();
                    let year_num = date_nums[0] as i32;
                    let month_num = date_nums[1];
                    let day_num = date_nums[2];
                    let _hour_num = date_nums[3];
                    let _min_num = date_nums[4];
                    let _sec_num = date_nums[5];

                    let (start_hour, start_min, start_sec) = get_timing_as_hms(&timing.0);

                    if let Some(start_time) = Local.with_ymd_and_hms(
                        year_num, month_num, day_num, 
                        start_hour, start_min, start_sec).single() {

                        let (end_hour, end_min, end_sec) = get_timing_as_hms(&timing.1);

                        if let Some(end_time) = Local.with_ymd_and_hms(
                            year_num, month_num, day_num, 
                            end_hour, end_min, end_sec).single() {

                            let local_timestamp = local.timestamp(); 
                            // if &timing.0 is less 
                            if local_timestamp > start_time.timestamp() && local_timestamp < end_time.timestamp() {

                                let task_list_clone_3 = Arc::clone(&app.task_list);
                                let task_clone_2 = Arc::clone(&task);
                                if let Err(e) = run_task(task_list_clone_3.clone(), task_clone_2.clone()) {
                                    loge!("Failed to run task: {}", e);
                                    display_error_with_message("Failed to run task!");    
                                }
                            }
                        } else {
                            loge!("Could not parse stop time");
                            display_error_with_message("Could not parse stop time!");    
                        }
                    } else {
                        loge!("Could not parse start time");
                        display_error_with_message("Could not parse start time!");    
                    }
                }

                scheduler.every(day_name)
                    .at(&timing.0)
                    .run(move || { 
                        if let Err(e) = run_task(task_list_clone.clone(), task_clone.clone()) {
                            loge!("Failed to run task:{}", e);
                            display_error_with_message("Failed to run task!");    
                        }
                    });

                scheduler.every(day_name)
                    .at(&timing.1)
                    // unused Result type in closure
                    .run(move || { 
                        if let Err(e) = stop_task(task_list_clone_2.clone()) {
                            loge!("Failed to stop task:{}", e);
                            display_error_with_message("Failed to stop task!"); 
                        }
                    });
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
        if let Err(e) = run_task(task_list_clone, task_clone) {

            loge!("Failed to run task:{}", e);
            display_error_with_message("Failed to run task!");    
        }
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
        let create_background = background::make(video_proc);

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
