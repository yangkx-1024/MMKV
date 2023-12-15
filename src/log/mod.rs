use std::fmt::{Debug, Display, Formatter};

pub mod logger;

/**
See [MMKV::set_logger](crate::MMKV::set_logger)
 */
pub trait Logger: Sync + Send + Debug {
    fn verbose(&self, log_str: String);
    fn info(&self, log_str: String);
    fn debug(&self, log_str: String);
    fn warn(&self, log_str: String);
    fn error(&self, log_str: String);
}

#[derive(Copy, Clone, Debug)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Verbose,
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            LogLevel::Error => "E",
            LogLevel::Warn => "W",
            LogLevel::Info => "I",
            LogLevel::Debug => "D",
            LogLevel::Verbose => "V",
            _ => "",
        };
        write!(f, "{str}")
    }
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
