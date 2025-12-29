use std::{
    error::Error,
    sync::{Arc, Mutex},
    thread,
    process::{
        Command
    },
    os::unix::process::CommandExt,

};
use crate::error::error_with_message as display_error_with_message;

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

/// This function takes the task to run and launches the correct software based on the variables 
/// set within the Task struct
pub fn run_task(task_list: Arc<Mutex<Vec<RunningTask>>>, task: Arc<Mutex<Task>>) -> Result<(), Box<dyn Error>> {
    let task_list_clone = Arc::clone(&task_list);
    let task_list_clone_two = Arc::clone(&task_list);

    logi!("Run task: {:?}", task.lock().unwrap());
    
    let model = task.lock().unwrap().model.clone();

    let looper = task.lock().unwrap().auto_loop.clone();

    let file_binding = task.lock().unwrap().file.clone();
    let file = String::from(file_binding.to_str().unwrap());
    let web_url = task.lock().unwrap().web_url.clone();
    let slide_delay = task.lock().unwrap().slide_delay.to_string();

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


    }
    // stop the task after launching the new task to ensure a smooh overlap
    logi!("Attempting to stop previous task");
    let _stopped_task = stop_task(task_list_clone_two.clone());
    Ok(())

}
