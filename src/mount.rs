use std::{
    error::Error,
    process::{
        Command,
        Stdio
    },
    path::{
        PathBuf
    },
    io::{
        Error as IoError,
        ErrorKind
    }
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

use regex::Regex;

use strum::Display;

#[derive(Display)]
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
    Unknown
}

impl Usb {
    fn as_device_path(&self) -> &'static str {
        match self {
            Usb::SDA1 => "/dev/sda1", 
            Usb::SDA2 => "/dev/sda2", 
            Usb::SDA3 => "/dev/sda3", 
            Usb::SDA4 => "/dev/sda4",
            Usb::SDB1 => "/dev/sdb1", 
            Usb::SDB2 => "/dev/sdb2", 
            Usb::SDB3 => "/dev/sdb3",
            Usb::SDB4 => "/dev/sdb4",
            Usb::SDC1 => "/dev/sdc1",
            Usb::SDC2 => "/dev/sdc2",
            Usb::SDC3 => "/dev/sdc3",
            Usb::SDC4 => "/dev/sdc4",
            Usb::Unknown => ""
        }
    }
}

pub fn identify_mounted_drives() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    logi!("Identifying mounted drives");
    let mut mounts = Vec::with_capacity(2);
    // find out if any drives mounted, otherwise default to /home/username
    let all_drives = Command::new("lsblk")
        .arg("-l")
        .arg("-o")
        .arg("NAME,HOTPLUG")
        .output()?;

    let all_drives_string = String::from_utf8_lossy(&all_drives.stdout);
    
    let re = Regex::new(r"sd[a,b,c][1-4]").unwrap();
    for line in all_drives_string.lines() {
        if re.is_match(line) {
            let drive_info = line.split(' ')
                .filter(|d| !d.is_empty() )
                .collect::<Vec<_>>();
                if drive_info[1] == "1" { 
                    
                    //To Remove:: have the thread sleep for one second as puppy umount sometimes fails
                    //let one_second = Duration::from_millis(1000); 
                    //thread::sleep(one_second);
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
                        &_ => Usb::Unknown

                    };

                    logi!("Storage drive {} matched", &drive);
    

                // check if device mounted
                let mut udc_info = Command::new("udisksctl")
                    .arg("info")
                    .arg("-b")
                    .arg(drive.as_device_path())
                    .stdout(Stdio::piped())
                    .spawn()?;

                let pipe = udc_info.stdout.take().unwrap();

                let udc_m_grep = Command::new("grep")
                    .arg("MountPoints")
                    .stdin(pipe)
                    .stdout(Stdio::piped())
                    .spawn()?;

                let udc_mounted_output = udc_m_grep.wait_with_output().expect("Failed to wait on grep");
                let _ = udc_info.wait();
                logi!("udisksctl and grep searched output successful");


                let udc_mounted_output = String::from_utf8_lossy(&udc_mounted_output.stdout);
                
                let mount_info = udc_mounted_output.split(" ")
                    .map(|x| x.trim())
                    .filter(|d| !d.is_empty() )
                    .collect::<Vec<_>>();

                // if the previous step has revealed that the partition is not mounted expect a 
                // vector of length=1 with "MountPoints" contained within.
                if mount_info.len() == 1 { 
                    // mount the device
                    let udc_output = Command::new("udisksctl")
                        .arg("mount")
                        .arg("-b")
                        .arg(drive.as_device_path())
                        .output()?;

                    let udc_output = String::from_utf8_lossy(&udc_output.stdout);

                    let mounted_drive_info = udc_output.split(" ")
                        .map(|x| x.trim())
                        .filter(|d| !d.is_empty() )
                        .collect::<Vec<_>>();
        
                    // this will be a vector with four parts
                    if mounted_drive_info.len() == 4 && !mounted_drive_info[3].is_empty() {
                        mounts.push(PathBuf::from(mounted_drive_info[3]));
                    }

                } else if mount_info.len() == 2 {
                    mounts.push(PathBuf::from(mount_info[1]));
                }
            }

        }
    }
    logi!("Returning all discovered mounts");
    Ok(mounts)
}

pub fn match_uuid(uuid: &str) -> Result<PathBuf, Box<dyn Error>> {
    logi!("Matching the storage device UUID");
    let all_drives = Command::new("lsblk")
        .arg("-l")
        .arg("-o")
        .arg("NAME,HOTPLUG,UUID,MOUNTPOINT")
        .output()?;

    let all_drives_string = String::from_utf8_lossy(&all_drives.stdout);
    
    for line in all_drives_string.lines() {
        if line.contains(uuid) {
            logi!("UUID matched to available drive");
            // the UUID matches, so get the path 
            let drive_info = line.split(' ')
                .filter(|d| !d.is_empty() )
                .collect::<Vec<_>>();

            if drive_info[2] == uuid { 
               return Ok(PathBuf::from(drive_info[3]));
            } else {
                logi!("UUID does not match drive info");
            }
        }
    }
    logw!("UUID could not be matched to existing storage UUIDs");
    Err(Box::new(IoError::new(ErrorKind::Other, "Could not match UUID")))
}
