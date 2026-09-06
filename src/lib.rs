//! Rust version of MMKV.
//! Examples:
//! ```
//! use mmkv::MMKV;
//!
//! let dir = std::env::temp_dir().join("mmkv_doc_lib");
//! std::fs::create_dir_all(&dir).unwrap();
//! let mmkv = MMKV::new(dir.to_str().unwrap(), #[cfg(feature = "encryption")] "88C51C536176AD8A8EE4A06F62EE897E").unwrap();
//! mmkv.put("key1", 1).unwrap();
//! assert_eq!(mmkv.get("key1"), Ok(1));
//! // Not actually needed unless you intend to delete all data
//! mmkv.clear_data().unwrap();
//! ```
//! For detailed API doc, see [MMKV]
//!
//! # Platform support
//!
//! The store is a `mmap` of the data file, kept in sync with `msync` and sized with
//! `ftruncate`, and the directory entries it renames are made durable with `fsync` on
//! the parent directory. That is a Unix contract, so the crate supports Unix-like
//! targets (Linux, Android, macOS, iOS) and nothing else. Windows is not supported.

// Fail here rather than at link time with a pile of missing `libc` symbols.
#[cfg(not(unix))]
compile_error!(
    "mmkv supports Unix-like targets only (Linux, Android, macOS, iOS): it maps the store \
     with mmap/msync and relies on fsync of the parent directory to publish renames."
);

pub use crate::core::buffer::{FromBytes, ProvideTypeToken, ToBytes, TypeToken};
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
    IOError(String),
    LockError(String),
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
        log!(crate::LogLevel::Error, $tag, $($arg)+)
    }
}

#[allow(unused_macros)]
macro_rules! warn {
    ($tag:expr, $($arg:tt)+) => {
        log!(crate::LogLevel::Warn, $tag, $($arg)+)
    }
}

macro_rules! info {
    ($tag:expr, $($arg:tt)+) => {
        log!(crate::LogLevel::Info, $tag, $($arg)+)
    }
}

macro_rules! debug {
    ($tag:expr, $($arg:tt)+) => {
        log!(crate::LogLevel::Debug, $tag, $($arg)+)
    }
}

macro_rules! verbose {
    ($tag:expr, $($arg:tt)+) => {
        log!(crate::LogLevel::Verbose, $tag, $($arg)+)
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
