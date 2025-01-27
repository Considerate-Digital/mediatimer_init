use std::{
    error::Error,
    process::Command,
    thread,
    time::Duration
};

use regex::Regex;

enum Usb {
    SDA1,
    SDA2,
    SDA3,
    SDA4,
    SDB1,
    SDB2,
    SDB3,
    SDB4,
    SDC1,
    SDC2,
    SDC3,
    SDC4,
    UNKNOWN
}

impl Usb {
    fn as_str(&self) -> &'static str {
        match self {
            Usb::SDA1 => "sda1", 
            Usb::SDA2 => "sda2", 
            Usb::SDA3 => "sda3", 
            Usb::SDA4 => "sda4",
            Usb::SDB1 => "sdb1", 
            Usb::SDB2 => "sdb2", 
            Usb::SDB3 => "sdb3",
            Usb::SDB4 => "sdb4",
            Usb::SDC1 => "sdc1",
            Usb::SDC2 => "sdc2",
            Usb::SDC3 => "sdc3",
            Usb::SDC4 => "sdc4",
            Usb::UNKNOWN => ""
        }
    }
}


pub fn find_mount_drives() -> Result<(), Box<dyn Error>> {
    // check with usbs are available 
    let all_drives = Command::new("lsblk")
        .arg("-l")
        .arg("-o")
        .arg("NAME,HOTPLUG")
        .output()
        .expect("some drives");
    
    let all_drives_string = String::from_utf8_lossy(&all_drives.stdout);
    
    for line in all_drives_string.lines() {
        let re = Regex::new(r"sd[a,b,c][1-4]").unwrap();
        if re.is_match(line) {
            let drive_info = line.split(' ')
                .filter(|d| *d != "" )
                .collect::<Vec<_>>();
                if drive_info[1] == "1" { 
                    // unmount the drive before going further
                    // have the thread sleep for one second as puppy umount sometimes fails
                    let one_second = Duration::from_millis(1000); 
                    thread::sleep(one_second);
                    let _unmount_com = Command::new("umount")
                        .arg("/dev/".to_owned() + drive_info[0])
                        .output()
                        .expect("Failed to unmount usb drive");

                    let drive = match drive_info[0] {
                        "sda1" => Usb::SDA1,
                        "sda2" => Usb::SDA2,
                        "sda3" => Usb::SDA3,
                        "sda4" => Usb::SDA4,
                        "sdb1" => Usb::SDB1,
                        "sdb2" => Usb::SDB2,
                        "sdb3" => Usb::SDB3,
                        "sdb4" => Usb::SDB4,
                        "sdc1" => Usb::SDC1,
                        "sdc2" => Usb::SDC2,
                        "sdc3" => Usb::SDC3,
                        "sdc4" => Usb::SDC4,
                        &_ => Usb::UNKNOWN

                    };
                    mount_usb(drive)?;
                }
        }
    }
    Ok(())
}

fn mount_usb(drive: Usb) -> Result<(), Box<dyn Error>> {

    let mnt_dir: String = match drive {
        Usb::SDA1 => format!("usb_{}", Usb::SDA1.as_str()),
        Usb::SDA2 => format!("usb_{}", Usb::SDA2.as_str()),
        Usb::SDA3 => format!("usb_{}", Usb::SDA3.as_str()),
        Usb::SDA4 => format!("usb_{}", Usb::SDA4.as_str()),
        Usb::SDB1 => format!("usb_{}", Usb::SDB1.as_str()),
        Usb::SDB2 => format!("usb_{}", Usb::SDB2.as_str()),
        Usb::SDB3 => format!("usb_{}", Usb::SDB3.as_str()),
        Usb::SDB4 => format!("usb_{}", Usb::SDB4.as_str()),
        Usb::SDC1 => format!("usb_{}", Usb::SDC1.as_str()),
        Usb::SDC2 => format!("usb_{}", Usb::SDC2.as_str()),
        Usb::SDC3 => format!("usb_{}", Usb::SDC3.as_str()),
        Usb::SDC4 => format!("usb_{}", Usb::SDC4.as_str()),
        Usb::UNKNOWN => "".to_string()

    };
    let drive_name = match drive {
        Usb::SDA1 => Usb::SDA1.as_str(), 
        Usb::SDA2 => Usb::SDA2.as_str(), 
        Usb::SDA3 => Usb::SDA3.as_str(), 
        Usb::SDA4 => Usb::SDA4.as_str(),
        Usb::SDB1 => Usb::SDB1.as_str(), 
        Usb::SDB2 => Usb::SDB2.as_str(), 
        Usb::SDB3 => Usb::SDB3.as_str(),
        Usb::SDB4 => Usb::SDB4.as_str(),
        Usb::SDC1 => Usb::SDC1.as_str(),
        Usb::SDC2 => Usb::SDC2.as_str(),
        Usb::SDC3 => Usb::SDC3.as_str(),
        Usb::SDC4 => Usb::SDC4.as_str(),
        Usb::UNKNOWN => ""
    };
    let _mount_drive = Command::new("mount")
            .arg("/dev/".to_owned() + drive_name)
            // tell mount to make the target dir
            .arg("-o")
            .arg("rw,x-mount.mkdir")
            .arg("/mnt/".to_owned() + &mnt_dir)
            .output()
            .expect("failed to mount");

    Ok(())
}


