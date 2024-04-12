use crate::core::io_looper::{Callback, IOLooper};
use chrono::{SecondsFormat, Utc};
use once_cell::sync::Lazy;
use std::fmt::Arguments;
use std::sync::atomic::{AtomicI32, Ordering};
use std::thread::ThreadId;
use std::{process, thread};

use crate::log::{LogLevel, Logger};
struct LogWrapper {
    io_looper: IOLooper,
}

struct LogWriter {
    inner_logger: Option<Box<dyn Logger>>,
}

impl LogWriter {
    fn write(&self, level: LogLevel, time: String, pid: u32, tid: ThreadId, log_str: String) {
        match &self.inner_logger {
            Some(_) => self.redirect(level, log_str),
            None => {
                println!("{} {}-{:?} {} {}", time, pid, tid, level, log_str)
            }
        }
    }

    fn redirect(&self, level: LogLevel, log_str: String) {
        let logger = self.inner_logger.as_ref().unwrap();
        match level {
            LogLevel::Error => logger.error(log_str),
            LogLevel::Warn => logger.warn(log_str),
            LogLevel::Info => logger.info(log_str),
            LogLevel::Debug => logger.debug(log_str),
            LogLevel::Verbose => logger.verbose(log_str),
            _ => {}
        }
    }
}

impl Callback for LogWriter {}

impl LogWrapper {
    fn new() -> Self {
        let log_writer = LogWriter { inner_logger: None };
        let io_looper = IOLooper::new(log_writer);
        LogWrapper { io_looper }
    }

    fn format_log(&self, level: LogLevel, tag: &str, content: Arguments) {
        let pid = process::id();
        let tid = thread::current().id();
        let time = local_time();
        let log_str = format!("[{}] {}", tag, content);
        // write log in io thread
        self.io_looper
            .post(move |callback| {
                let writer = callback.downcast_ref::<LogWriter>().unwrap();
                writer.write(level, time, pid, tid, log_str);
            })
            .unwrap();
    }
}

static LOG_WRAPPER: Lazy<LogWrapper> = Lazy::new(LogWrapper::new);

pub fn log(level: LogLevel, tag: &str, args: Arguments) {
    if get_log_level() < level as i32 {
        return;
    }
    LOG_WRAPPER.format_log(level, tag, args);
}

fn local_time() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, false)
}

static LOG_LEVEL: AtomicI32 = AtomicI32::new(5);

pub fn set_log_level(level: LogLevel) {
    LOG_LEVEL.swap(level as i32, Ordering::Release);
}

pub fn get_log_level() -> i32 {
    LOG_LEVEL.load(Ordering::Acquire)
}

pub fn set_logger(log_impl: Option<Box<dyn Logger>>) {
    // Here move the log_impl to io thread
    LOG_WRAPPER
        .io_looper
        .post(|callback| {
            let writer = callback.downcast_mut::<LogWriter>().unwrap();
            drop(writer.inner_logger.take());
            writer.inner_logger = log_impl;
        })
        .unwrap();
}

#[allow(dead_code)]
pub fn sync() {
    LOG_WRAPPER.io_looper.sync()
}
