//! Rust version of MMKV.
//! Examples:
//! ```
//! use mmkv::MMKV;
//!
//! let temp_dir = std::env::temp_dir();
//! MMKV::initialize(temp_dir.to_str().unwrap(), #[cfg(feature = "encryption")] "88C51C536176AD8A8EE4A06F62EE897E");
//! MMKV::put_i32("key1", 1).unwrap();
//! assert_eq!(MMKV::get_i32("key1"), Ok(1));
//! // Not actually needed unless you intend to delete all data
//! MMKV::clear_data();
//! ```
//! For detailed API doc, see [MMKV]
pub use crate::log::LogLevel;
pub use crate::log::Logger;
pub use crate::mmkv::MMKV;

#[derive(Debug, PartialEq)]
pub enum Error {
    KeyNotFound,
    DecodeFailed(String),
    TypeMissMatch,
    DataInvalid,
    InstanceClosed,
    EncodeFailed(String),
    #[cfg(feature = "encryption")]
    DecryptFailed(String),
    #[cfg(feature = "encryption")]
    EncryptFailed(String),
}

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! log {
    ($level:expr, $tag:expr, $($arg:tt)+) => {
        crate::log::logger::log($level, $tag, format_args!($($arg)+))
    }
}

macro_rules! error {
    ($tag:expr, $($arg:tt)+) => {
        if crate::log::logger::get_log_level() >= crate::log::LogLevel::Error as i32 {
            log!(crate::LogLevel::Error, $tag, $($arg)+)
        }
    }
}

#[allow(unused_macros)]
macro_rules! warn {
    ($tag:expr, $($arg:tt)+) => {
        if crate::log::logger::get_log_level() >= crate::log::LogLevel::Warn as i32 {
            log!(crate::LogLevel::Warn, $tag, $($arg)+)
        }
    }
}

macro_rules! info {
    ($tag:expr, $($arg:tt)+) => {
        if crate::log::logger::get_log_level() >= crate::log::LogLevel::Info as i32 {
            log!(crate::LogLevel::Info, $tag, $($arg)+)
        }
    }
}

macro_rules! debug {
    ($tag:expr, $($arg:tt)+) => {
        if crate::log::logger::get_log_level() >= crate::log::LogLevel::Debug as i32 {
            log!(crate::LogLevel::Debug, $tag, $($arg)+)
        }
    }
}

macro_rules! verbose {
    ($tag:expr, $($arg:tt)+) => {
        if crate::log::logger::get_log_level() >= crate::log::LogLevel::Verbose as i32 {
            log!(crate::LogLevel::Verbose, $tag, $($arg)+)
        }
    }
}

mod core;
#[cfg(not(target_os = "android"))]
#[cfg(not(feature = "encryption"))]
/// Expose the C API
mod ffi;
#[cfg(target_os = "android")]
/// Expose the JNI interface for android
mod jni;
mod log;
mod mmkv;
