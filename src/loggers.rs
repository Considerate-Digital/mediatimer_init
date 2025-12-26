use log::{LevelFilter};
use std::error::Error;
use systemd_journal_logger::JournalLog;

pub fn setup_logger() -> Result<(), Box<dyn Error>> {
    JournalLog::new()?.install()?;
    log::set_max_level(LevelFilter::Info);
    Ok(())
}

#[macro_export] 
macro_rules! logi {
    ($($t:tt)*) => {{
       info!($($t)*); 
    }};
}

#[macro_export] 
macro_rules! logw {
    ($($t:tt)*) => {{
       warn!($($t)*); 
    }};
}

#[macro_export] 
macro_rules! loge {
    ($($t:tt)*) => {{
       error!($($t)*); 
       eprintln!($($t)*);
    }};
}


