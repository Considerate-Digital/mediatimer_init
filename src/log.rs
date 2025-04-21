use log::{info, warn, error, LevelFilter};

use systemd_journal_logger::JournalLog;

JournalLog::new().unwrap().install().unwrap();
log::set_max_level(LevelFilter::Info);

pub fn log_info(message: &str) {
   info!(message); 
}

pub fn log_warn(message: &str) {
    warn!(message);
}

pub fn log_error(message: &str) {
    error!(message);
}

