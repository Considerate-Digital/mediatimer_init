use std::{
    error::Error,
    sync::{Arc, Mutex},
    thread,
    process::{
        Command
    },
    os::unix::process::CommandExt,

};

use crate::{
    logi,
    loge,
    logw
};
use log::{
    info,
    warn,
    error
};

use crate::{
    RunningTask,
    Task,
    Autoloop,
    ProcType,
    Model,
    stop_task
};

use chrono::{
    Local,
    DateTime
};



fn get_seek_seconds(start_time: &str) -> Result<String, Box<dyn Error>> {
    let local = Local::now();
    let now_timestamp = local.timestamp();

    let date_string = format!("{}", local.format("%Y:%m:%d:%H:%M:%S:%z"));

    let date_nums: Vec<&str> = date_string.split(":").collect::<Vec<&str>>();
    let year_num = date_nums[0];
    let month_num = date_nums[1];
    let day_num = date_nums[2];
    let _hour_num = date_nums[3];
    let _min_num = date_nums[4];
    let _sec_num = date_nums[5];
    let timezone = date_nums[6];
    let formatted_date = format!("{}:{}:{}:{} {}", year_num, month_num, day_num, start_time, timezone);
    let start_dt = DateTime::parse_from_str(&formatted_date, "%Y:%m:%d:%H:%M:%S %z")?;

    let start_timestamp = start_dt.timestamp();

    if now_timestamp > start_timestamp {
        let time_diff = (now_timestamp - start_timestamp).to_string();
        logi!("Time Difference: {}", time_diff);
        Ok(time_diff)
    } else {
        Ok(String::from("0"))
    }
}

/// This function takes the task to run and launches the correct software based on the variables 
/// set within the Task struct
pub fn run_task(task_list: Arc<Mutex<Vec<RunningTask>>>, task: Arc<Mutex<Task>>, start_time: &str) -> Result<(), Box<dyn Error>> {
    let task_list_clone = Arc::clone(&task_list);
    let task_list_clone_two = Arc::clone(&task_list);

    logi!("Run task: {:?}", task.lock().unwrap());
    
    let model = task.lock().unwrap().model.clone();

    let looper = task.lock().unwrap().auto_loop.clone();

    let file_binding = task.lock().unwrap().file.clone();
    let file = String::from(file_binding.to_str().unwrap());
    let web_url = task.lock().unwrap().web_url.clone();
    let slide_delay = task.lock().unwrap().slide_delay.to_string();

    // get seek seconds
    let mut seek_seconds: String = String::new();
    if start_time != "" {
        seek_seconds = get_seek_seconds(start_time)?;
    }

    if model == Model::Eco {
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

                            let running_task = RunningTask::new(child, false );
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
                                .arg("-ss")
                                .arg(seek_seconds)
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
                                .arg("-ss")
                                .arg(seek_seconds)
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

    } else {
        // standard and pro features are identical for mediatimer_init
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
                                .arg("-ss")
                                .arg(seek_seconds)
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
                                .arg("-ss")
                                .arg(seek_seconds)
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


    }
    // stop the task after launching the new task to ensure a smooh overlap
    logi!("Attempting to stop previous task");
    let _stopped_task = stop_task(task_list_clone_two.clone());
    Ok(())

}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{
        Local,
        TimeDelta
    };

    // Test the seek second fn
    #[test]
    fn test_seek_seconds() {
        // create start time from now, plus 30 seconds
        let date_string = Local::now().checked_add_signed(TimeDelta::new(30, 0).unwrap()).unwrap();
        let start_time = format!("{}", date_string.format("%H:%M:%S"));
        let seek_seconds = get_seek_seconds(&start_time).unwrap();
        assert_eq!(seek_seconds, String::from("0"));

        // create start time from now, minus 30 seconds
        let date_string = Local::now().checked_add_signed(TimeDelta::new(-30, 0).unwrap()).unwrap();
        let start_time = format!("{}", date_string.format("%H:%M:%S"));
        let seek_seconds = get_seek_seconds(&start_time).unwrap();
        assert_eq!(seek_seconds, String::from("30"));
    }
}

