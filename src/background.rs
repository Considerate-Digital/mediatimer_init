use std::{
    process::Command,
    thread,
    sync::{
        Mutex,
        Arc
    }
};

use crate::RunningTask;

pub fn make() {
    let _made_background = Command::new("ffmpeg")
        .arg("-f")
        .arg("lavfi")
        .arg("-y")
        .arg("-i")
        .arg("color=black:s=1920x1080:r=10")
        .arg("-t")
        .arg("1")
        .arg("/tmp/black.mp4")
        .output()
        .expect("Could not create black video");
}

pub fn run(task_list: Arc<Mutex<Vec<RunningTask>>>) {
    thread::spawn(move || {
        let child = Command::new("ffplay")
            .arg("-fs")
            .arg("-loop")
            .arg("-1")
            .arg("/tmp/black.mp4")
            .spawn()
            .expect("no child");

        let running_task = RunningTask::new(child, true);
        task_list.lock().unwrap().push(running_task);
    });
}
