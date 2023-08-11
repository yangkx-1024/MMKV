use crate::log::{LogLevel, Logger};
use std::fmt::{Arguments, Debug};
use std::sync::atomic::{AtomicI32, AtomicPtr, Ordering};
use std::sync::OnceLock;

const LOG_TAG: &str = "MMKV:LOG";

#[derive(Debug)]
struct DefaultLogger;

impl Logger for DefaultLogger {
    fn verbose(&self, log_str: &str) {
        println!("V - {log_str}");
    }

    fn info(&self, log_str: &str) {
        println!("I - {log_str}");
    }

    fn debug(&self, log_str: &str) {
        println!("D - {log_str}");
    }

    fn warn(&self, log_str: &str) {
        println!("W - {log_str}");
    }

    fn error(&self, log_str: &str) {
        println!("E - {log_str}");
    }
}

static DEFAULT_LOG_IMPL: OnceLock<Box<dyn Logger>> = OnceLock::new();
static LOG_IMPL: AtomicPtr<Box<dyn Logger>> = AtomicPtr::new(std::ptr::null_mut());

fn inner_logger() -> &'static Box<dyn Logger> {
    let p = LOG_IMPL.load(Ordering::Acquire);
    if p != std::ptr::null_mut() {
        unsafe { p.as_ref().unwrap() }
    } else {
        DEFAULT_LOG_IMPL.get_or_init(|| Box::new(DefaultLogger))
    }
}

pub fn log(level: LogLevel, tag: &str, args: Arguments) {
    let level_int = LOG_LEVEL.load(Ordering::Acquire);
    if level as i32 > level_int {
        return;
    }
    match level {
        LogLevel::Error => inner_logger().error(&format!("{tag} - {}", args)),
        LogLevel::Warn => inner_logger().warn(&format!("{tag} - {}", args)),
        LogLevel::Info => inner_logger().info(&format!("{tag} - {}", args)),
        LogLevel::Debug => inner_logger().debug(&format!("{tag} - {}", args)),
        LogLevel::Verbose => inner_logger().verbose(&format!("{tag} - {}", args)),
        _ => {}
    }
}

static LOG_LEVEL: AtomicI32 = AtomicI32::new(5);

pub fn set_log_level(level: i32) {
    let old_level = LOG_LEVEL.swap(level, Ordering::Release);
    if old_level != level {
        debug!(LOG_TAG, "update log level from {} to {}", old_level, level)
    }
}

pub fn set_logger(log_impl: Box<dyn Logger>) {
    let log_str = format!("set new logger: {:?}", log_impl);
    let raw_ptr = Box::into_raw(Box::new(log_impl));
    let old_log_impl = LOG_IMPL.swap(raw_ptr, Ordering::Release);
    if old_log_impl != std::ptr::null_mut() {
        unsafe {
            drop(Box::from_raw(old_log_impl));
        }
    }
    debug!(LOG_TAG, "{}", log_str);
}

pub fn reset() {
    set_log_level(5);
    let old_log_impl = LOG_IMPL.swap(std::ptr::null_mut(), Ordering::Release);
    if old_log_impl != std::ptr::null_mut() {
        unsafe {
            drop(Box::from_raw(old_log_impl));
        }
    }
}