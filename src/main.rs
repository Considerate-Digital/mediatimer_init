use std::{
    io,
    thread,
    time::Duration,
    path::Path,
    env,
    error::Error,
    process::Command,
};
use regex::Regex;

fn mount_usb() -> Result<(), Box<dyn Error>> {
    
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    // use this dir .env for testing
    dotenvy::from_path(Path::new("/home/alex/medialoop/src/.env"))?;

    let usb_mounts = vec![
        ("sda1", "usb_stick_1"), 
        ("sda2", "usb_stick_1_2"), 
        ("sdb1", "usb_stick_2"),
        ("sdb2", "usb_stick_2_2"), 
        ("sdc1", "usb_stick_3"),
        ("sdc2", "usb_stick_3_3")
    ];

    // check which usbs are mounted
    /*
    let usb_drives = Command::new("grep")
        .arg("-i")
        .arg("-E")
        .arg("/dev/sd(a|b|c|d)")
        .arg("/proc/mounts")
        .output()
        .expect("failed to check for usb drives");

    println!("{:?}", usb_drives);
    */

    // check with usbs are available 
    let all_drives = Command::new("lsblk")
        .arg("-l")
        .output()
        .expect("some drives");
    let all_drives_string = String::from_utf8_lossy(&all_drives.stdout);
    //println!("{}", all_drives_string);
    let mut usb_no = 1;
    for line in all_drives_string.lines() {
        let re = Regex::new(r"sda[1-5]").unwrap();
        if re.is_match(line) {
            println!("{:?}", line);
            mount_usb()
        }
    }

    //for 

    // setup mounts
    for (drive, name) in usb_mounts {
        /*
        let setup_dir = Command::new("mkdir")
            .arg("/mnt/".to_owned() + point)
            .output()
            .expect("failed to create dir");
        println!("{:?}", setup_dir);
        */
        /*
        let mount = Command::new("mount")
            .arg("/dev/".to_owned() + drive)
            // tell mount to make the target dir
            .arg("-o")
            .arg("rw,x-mount.mkdir")
            .arg("/mnt/".to_owned() + name)
            .output()
            .expect("failed to mount");
        println!("{:?}",mount);
        */
    }     
    
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
