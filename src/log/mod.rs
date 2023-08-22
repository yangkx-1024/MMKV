use std::fmt::Debug;

pub mod logger;

/**
See [MMKV::set_logger](crate::MMKV::set_logger)
 */
pub trait Logger: Sync + Send + Debug {
    fn verbose(&self, log_str: &str);
    fn info(&self, log_str: &str);
    fn debug(&self, log_str: &str);
    fn warn(&self, log_str: &str);
    fn error(&self, log_str: &str);
}

#[derive(Copy, Clone)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Verbose,
}

impl TryFrom<i32> for LogLevel {
    type Error = ();

    fn try_from(v: i32) -> Result<Self, <LogLevel as TryFrom<i32>>::Error> {
        match v {
            v if v == LogLevel::Off as i32 => Ok(LogLevel::Off),
            v if v == LogLevel::Error as i32 => Ok(LogLevel::Error),
            v if v == LogLevel::Warn as i32 => Ok(LogLevel::Warn),
            v if v == LogLevel::Info as i32 => Ok(LogLevel::Info),
            v if v == LogLevel::Debug as i32 => Ok(LogLevel::Debug),
            v if v == LogLevel::Verbose as i32 => Ok(LogLevel::Verbose),
            _ => Err(()),
        }
    }
}
