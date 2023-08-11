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
