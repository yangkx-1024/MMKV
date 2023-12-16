use chrono::{SecondsFormat, Utc};
use std::fmt::{Arguments, Debug};
use std::ops::Deref;
use std::sync::atomic::{AtomicI32, AtomicPtr, Ordering};
use std::sync::OnceLock;
use std::{process, thread};

use crate::log::{LogLevel, Logger};

#[derive(Debug)]
struct DefaultLogger;

impl DefaultLogger {
    fn local_time(&self) -> String {
        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, false)
    }

    fn format_log(&self, level: &str, str: String) {
        let thread = thread::current();
        println!(
            "{} {}-{:?} {} {}",
            self.local_time(),
            process::id(),
            thread.id(),
            level,
            str
        )
    }
}

impl Logger for DefaultLogger {
    fn verbose(&self, log_str: String) {
        self.format_log("V", log_str);
    }

    fn info(&self, log_str: String) {
        self.format_log("I", log_str);
    }

    fn debug(&self, log_str: String) {
        self.format_log("D", log_str)
    }

    fn warn(&self, log_str: String) {
        self.format_log("W", log_str)
    }

    fn error(&self, log_str: String) {
        self.format_log("E", log_str)
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
    if get_log_level() < level as i32 {
        return;
    }
    let str = format!("[{}] {}", tag, args);
    match level {
        LogLevel::Error => inner_logger().error(str),
        LogLevel::Warn => inner_logger().warn(str),
        LogLevel::Info => inner_logger().info(str),
        LogLevel::Debug => inner_logger().debug(str),
        LogLevel::Verbose => inner_logger().verbose(str),
        _ => {}
    }
}

static LOG_LEVEL: AtomicI32 = AtomicI32::new(5);

pub fn set_log_level(level: LogLevel) {
    LOG_LEVEL.swap(level as i32, Ordering::Release);
}

pub fn get_log_level() -> i32 {
    LOG_LEVEL.load(Ordering::Acquire)
}

pub fn set_logger(log_impl: Box<dyn Logger>) {
    set_raw_logger(Box::into_raw(Box::new(log_impl)));
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
