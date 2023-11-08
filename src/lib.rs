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

pub use crate::log::Logger;

pub use crate::log::LogLevel;

macro_rules! log {
    ($level:expr, $tag:expr, $($arg:tt)+) => {{
        crate::log::logger::log($level, $tag, format_args!($($arg)+))
    }}
}

macro_rules! error {
    ($tag:expr, $($arg:tt)+) => {{
        log!(crate::LogLevel::Error, $tag, $($arg)+)
    }}
}

#[allow(unused_macros)]
macro_rules! warn {
    ($tag:expr, $($arg:tt)+) => {{
        log!(crate::LogLevel::Warn, $tag, $($arg)+)
    }}
}

macro_rules! info {
    ($tag:expr, $($arg:tt)+) => {{
        log!(crate::LogLevel::Info, $tag, $($arg)+)
    }}
}

macro_rules! debug {
    ($tag:expr, $($arg:tt)+) => {{
        log!(crate::LogLevel::Debug, $tag, $($arg)+)
    }}
}

macro_rules! verbose {
    ($tag:expr, $($arg:tt)+) => {{
        log!(crate::LogLevel::Verbose, $tag, $($arg)+)
    }}
}

mod core;
#[cfg(target_os = "android")]
mod jni;
#[cfg(any(target_os = "ios", target_os = "macos"))]
mod clib;
mod log;
mod mmkv;
