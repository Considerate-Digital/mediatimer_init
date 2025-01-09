use std::{
    io,
    thread,
    time::Duration,
    path::Path,
    env,
    error::Error,
    process::Command,
};

fn main() -> Result<(), Box<dyn Error>> {
    // use this dir .env for testing
    dotenvy::from_path(Path::new("/home/alex/medialoop/src/.env"))?;
    
    let mut file_path = String::with_capacity(20);
    for (key, value) in env::vars() {
        match key.as_str() {
            "ML_WEEKDAYS" => println!("{}", value),
            "ML_START" => println!("{}", value),
            "ML_END" => println!("{}", value),
            "ML_FILE" => file_path.push_str(value.as_str()),
            _ => {}
        }
    }

    let output = Command::new("cvlc")
        .arg("-fL")
        .arg("--no-video-title-show")
        .arg(&file_path)
        .output()
        .expect("failed to start video");

    // create custom command


    Ok(())
}
