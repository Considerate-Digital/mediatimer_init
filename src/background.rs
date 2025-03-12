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

pub fn make() {
    let username = whoami::username();
    let env_dir_path: PathBuf =["/home/", &username, ".mediatimer_config/black.mp4"].iter().collect();
    // TODO error handling
    if !env_dir_path.exists() {
        let path_str = env_dir_path.to_str().unwrap();
        let _made_background = Command::new("ffmpeg")
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
    let path_str = env_dir_path.to_str().unwrap();

    let child = Command::new("ffplay")
        .arg("-fs")
        .arg("-loop")
        .arg("-1")
        .arg(path_str)
        .spawn()
        .expect("no child");

    let running_task = RunningTask::new(child, true);
    task_list.lock().unwrap().push(running_task);
}
