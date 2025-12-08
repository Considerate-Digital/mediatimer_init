use std::{
    path::PathBuf,
    process::Command,
    sync::{
        Mutex,
        Arc
    }
};

use crate::RunningTask;
use crate::ProcType;
use crate::Task;

use crate::error::error_with_message as display_error_with_message;

pub fn make(proc_type: ProcType) {
    let username = whoami::username();
    let env_dir_path: PathBuf =["/home/", &username, ".mediatimer_config/black.mp4"].iter().collect();
    if let Some(path_str) = env_dir_path.to_str() {
        if proc_type == ProcType::Audio {
            let _made_background_audio = Command::new("ffmpeg")
                .arg("-hide_banner")
                .arg("-loglevel")
                .arg("error")
                .arg("-f")
                .arg("lavfi")
                .arg("-i")
                .arg("aevalsrc=0:s=48000:n=1920:d=4.0")
                .arg(path_str)
                .output()
                .expect("Could not create empty audio");

        } else {
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
                .output()
                .expect("Could not create black video");
        }
    } else {
        display_error_with_message("Unable to locate config");
    };
}

pub fn run(task_list: Arc<Mutex<Vec<RunningTask>>>) {
    let username = whoami::username();
    let env_dir_path: PathBuf =["/home/", &username, ".mediatimer_config/black.mp4"].iter().collect();

    let background_task = Task::background();
    if let Some(path_str) = env_dir_path.to_str() {
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
        let running_task = RunningTask::new(child, true, Arc::new(Mutex::new(background_task)));
        task_list.lock().unwrap().push(running_task);
    } else {
        display_error_with_message("Unable to locate config");
    };
}
