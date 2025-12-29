use std::{
    error::Error,
    path::PathBuf,
    process::Command,
    sync::{
        Mutex,
        Arc
    }
};
use log::{
    error,
    info,
    warn
};
use crate::{
    loge,
    logi,
    logw
};

use crate::RunningTask;

use crate::error::error_with_message as display_error_with_message;

pub fn make() -> Result<(), Box<dyn Error>> {
    logi!("Making background");
    let username = whoami::username();
    let env_dir_path: PathBuf =["/home/", &username, ".mediatimer_config/black.mp4"].iter().collect();
    if let Some(path_str) = env_dir_path.to_str() {
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
            .arg("2")
            .arg(path_str)
            .output()?;
    } else {
        loge!("Failed to convert background path to str"); 
        display_error_with_message("Failed to convert background path to str");
    };
    Ok(())
}

pub fn run(task_list: Arc<Mutex<Vec<RunningTask>>>) -> Result<(), Box<dyn Error>> {
    logi!("Attempting to run background");
    let username = whoami::username();
    let env_dir_path: PathBuf =["/home/", &username, ".mediatimer_config/black.mp4"].iter().collect();

    if let Some(path_str) = env_dir_path.to_str() {
        let child = Command::new("ffplay")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-fs")
            .arg("-loop")
            .arg("-1")
            .arg(path_str)
            .spawn()?;

        let running_task = RunningTask::new(child, true);
        task_list.lock().unwrap().push(running_task)
    } else {
        loge!("Failed to convert background path to str"); 
        display_error_with_message("Failed to convert background path to str");
    };
    Ok(())
}
