use std::fmt::{Arguments, Debug};
use std::ops::Deref;
use std::sync::atomic::{AtomicI32, AtomicPtr, Ordering};
use std::sync::OnceLock;

use crate::log::{LogLevel, Logger};

const LOG_TAG: &str = "MMKV:LOG";

#[derive(Debug)]
struct DefaultLogger;

impl Logger for DefaultLogger {
    fn verbose(&self, log_str: String) {
        println!("V - {log_str}");
    }

    fn info(&self, log_str: String) {
        println!("I - {log_str}");
    }

    fn debug(&self, log_str: String) {
        println!("D - {log_str}");
    }

    fn warn(&self, log_str: String) {
        println!("W - {log_str}");
    }

    fn error(&self, log_str: String) {
        println!("E - {log_str}");
    }
}

static DEFAULT_LOG_IMPL: OnceLock<Box<dyn Logger>> = OnceLock::new();
static LOG_IMPL: AtomicPtr<Box<dyn Logger>> = AtomicPtr::new(std::ptr::null_mut());

fn inner_logger() -> &'static dyn Logger {
    let p = LOG_IMPL.load(Ordering::Acquire);
    if !p.is_null() {
        unsafe { p.as_ref().unwrap() }.deref()
    } else {
        DEFAULT_LOG_IMPL
            .get_or_init(|| Box::new(DefaultLogger))
            .deref()
    }
}

pub fn log(level: LogLevel, tag: &str, args: Arguments) {
    match level {
        LogLevel::Error => inner_logger().error(format!("{tag} - {}", args)),
        LogLevel::Warn => inner_logger().warn(format!("{tag} - {}", args)),
        LogLevel::Info => inner_logger().info(format!("{tag} - {}", args)),
        LogLevel::Debug => inner_logger().debug(format!("{tag} - {}", args)),
        LogLevel::Verbose => inner_logger().verbose(format!("{tag} - {}", args)),
        _ => {}
    }
}

static LOG_LEVEL: AtomicI32 = AtomicI32::new(5);

pub fn set_log_level(level: LogLevel) {
    let level = level as i32;
    let old_level = LOG_LEVEL.swap(level, Ordering::Release);
    if old_level != level {
        debug!(LOG_TAG, "update log level from {} to {}", old_level, level)
    }
}

pub fn get_log_level() -> i32 {
    LOG_LEVEL.load(Ordering::Acquire)
}

pub fn set_logger(log_impl: Box<dyn Logger>) {
    let log_str = format!("set new logger: {:?}", log_impl);
    set_raw_logger(Box::into_raw(Box::new(log_impl)));
    debug!(LOG_TAG, "{}", log_str);
}

pub fn reset() {
    set_log_level(LogLevel::Verbose);
    set_raw_logger(std::ptr::null_mut());
}

fn set_raw_logger(logger: *mut Box<dyn Logger>) {
    let old_log_impl = LOG_IMPL.swap(logger, Ordering::Release);
    if !old_log_impl.is_null() {
        unsafe {
            drop(Box::from_raw(old_log_impl));
        }
    }
}
