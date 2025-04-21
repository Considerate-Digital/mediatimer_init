use std::{
    path::PathBuf,
    process::Command,
    thread,
    sync::{
        Mutex,
        Arc
    }
};

use crate::RunningTask;

mod error;
use crate::error::error as display_error;
use crate::error::error_with_message as display_error_with_message;



pub fn make() {
    let username = whoami::username();
    let env_dir_path: PathBuf =["/home/", &username, ".mediatimer_config/black.mp4"].iter().collect();
    if !env_dir_path.exists() {
        let path_str = env_dir_path.to_str().unwrap_or_else(|_| {
            let error_message = format!("Could not find config file path.");
           error_log(&error_message);
           display_error_with_message(&error_message);
       });

        let _made_background = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-f")
            .arg("lavfi")
            .arg("-y")
            .arg("-i")
            .arg("color=black:s=1920x1080:r=1")
            .arg("-t")
            .arg("30")
            .arg(path_str)
            .output()
            .expect("Could not create black video");
    }
    
}

pub fn run(task_list: Arc<Mutex<Vec<RunningTask>>>) {
    let username = whoami::username();
    let env_dir_path: PathBuf =["/home/", &username, ".mediatimer_config/black.mp4"].iter().collect();

    // TODO error handling
    let path_str = env_dir_path.to_str().unwrap_or_else(|_| {
            let error_message = format!("Could not find config file path.");
           error_log(&error_message);
           display_error_with_message(&error_message);
       });
    let child = Command::new("ffplay")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")

        .arg("-fs")
        .arg("-loop")
        .arg("-1")
        .arg(path_str)
        .spawn()
        .expect("no child");

    let running_task = RunningTask::new(child, true);
    task_list.lock().unwrap().push(running_task);
}
