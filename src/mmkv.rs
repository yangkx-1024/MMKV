use crate::core::buffer::Buffer;
use crate::core::config::Config;
use crate::core::mmkv_impl::MmkvImpl;
use crate::log::logger;
use crate::Error::InstanceClosed;
use crate::{LogLevel, Result};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

const DEFAULT_FILE_NAME: &str = "mini_mmkv";
const PAGE_SIZE: u64 = 4 * 1024; // 4KB is the default Linux page size

static MMKV_INSTANCE: RwLock<Option<MmkvImpl>> = RwLock::new(None);

pub struct MMKV;

macro_rules! mmkv_get {
    ($key:expr, $decode:ident) => {
        match MMKV_INSTANCE.read().unwrap().as_ref() {
            Some(mmkv) => mmkv.get($key)?.$decode(),
            None => Err(InstanceClosed),
        }
    };
}

macro_rules! mmkv_put {
    ($key:expr, $value:expr) => {
        match MMKV_INSTANCE.write().unwrap().as_mut() {
            Some(mmkv) => mmkv.put($key, $value),
            None => Err(InstanceClosed),
        }
    };
}

macro_rules! mut_mmkv_call {
    ($op:ident) => {
        MMKV_INSTANCE
            .write()
            .unwrap()
            .as_mut()
            .map(|mmkv| mmkv.$op())
    };
}

impl MMKV {
    /**
    Initialize the MMKV instance with a writeable directory,
    absolute or relative paths are acceptable.

    All API calls before initialization will panic.

    Calling [initialize](MMKV::initialize) multiple times is allowed,
    the old instance will be closed (see [close](MMKV::close)), the last call will take over.

    If enabled feature "encryption", additional param `key` is required,
    the key should be a hexadecimal string of length 16, for example:

    `88C51C536176AD8A8EE4A06F62EE897E`
     */
    pub fn initialize(dir: &str, #[cfg(feature = "encryption")] key: &str) {
        let mut instance = MMKV_INSTANCE.write().unwrap();
        drop(instance.take());
        let file_path = MMKV::resolve_file_path(dir);
        let config = Config::new(file_path.as_path(), PAGE_SIZE);
        *instance = Some(MmkvImpl::new(
            config,
            #[cfg(feature = "encryption")]
            key,
        ));
    }

    fn resolve_file_path(dir: &str) -> PathBuf {
        let path = Path::new(dir);
        if !path.is_dir() {
            panic!("path {}, is not dir", dir);
        }
        let metadata = path
            .metadata()
            .unwrap_or_else(|_| panic!("failed to get attr of dir {}", dir));
        if metadata.permissions().readonly() {
            panic!("path {}, is readonly", dir);
        }
        path.join(DEFAULT_FILE_NAME)
    }

    pub fn put_str(key: &str, value: &str) -> Result<()> {
        mmkv_put!(key, Buffer::from_str(key, value))
    }

    pub fn get_str(key: &str) -> Result<String> {
        mmkv_get!(key, decode_str)
    }

    pub fn put_i32(key: &str, value: i32) -> Result<()> {
        mmkv_put!(key, Buffer::from_i32(key, value))
    }

    pub fn get_i32(key: &str) -> Result<i32> {
        mmkv_get!(key, decode_i32)
    }

    pub fn put_bool(key: &str, value: bool) -> Result<()> {
        mmkv_put!(key, Buffer::from_bool(key, value))
    }

    pub fn get_bool(key: &str) -> Result<bool> {
        mmkv_get!(key, decode_bool)
    }

    pub fn put_i64(key: &str, value: i64) -> Result<()> {
        mmkv_put!(key, Buffer::from_i64(key, value))
    }

    pub fn get_i64(key: &str) -> Result<i64> {
        mmkv_get!(key, decode_i64)
    }

    pub fn put_f32(key: &str, value: f32) -> Result<()> {
        mmkv_put!(key, Buffer::from_f32(key, value))
    }

    pub fn get_f32(key: &str) -> Result<f32> {
        mmkv_get!(key, decode_f32)
    }

    pub fn put_f64(key: &str, value: f64) -> Result<()> {
        mmkv_put!(key, Buffer::from_f64(key, value))
    }

    pub fn get_f64(key: &str) -> Result<f64> {
        mmkv_get!(key, decode_f64)
    }

    pub fn put_byte_array(key: &str, value: &[u8]) -> Result<()> {
        mmkv_put!(key, Buffer::from_byte_array(key, value))
    }

    pub fn get_byte_array(key: &str) -> Result<Vec<u8>> {
        mmkv_get!(key, decode_byte_array)
    }

    pub fn put_i32_array(key: &str, value: &[i32]) -> Result<()> {
        mmkv_put!(key, Buffer::from_i32_array(key, value))
    }

    pub fn get_i32_array(key: &str) -> Result<Vec<i32>> {
        mmkv_get!(key, decode_i32_array)
    }

    pub fn put_i64_array(key: &str, value: &[i64]) -> Result<()> {
        mmkv_put!(key, Buffer::from_i64_array(key, value))
    }

    pub fn get_i64_array(key: &str) -> Result<Vec<i64>> {
        mmkv_get!(key, decode_i64_array)
    }

    pub fn put_f32_array(key: &str, value: &[f32]) -> Result<()> {
        mmkv_put!(key, Buffer::from_f32_array(key, value))
    }

    pub fn get_f32_array(key: &str) -> Result<Vec<f32>> {
        mmkv_get!(key, decode_f32_array)
    }

    pub fn put_f64_array(key: &str, value: &[f64]) -> Result<()> {
        mmkv_put!(key, Buffer::from_f64_array(key, value))
    }

    pub fn get_f64_array(key: &str) -> Result<Vec<f64>> {
        mmkv_get!(key, decode_f64_array)
    }

    /**
    Clear all data and [close](MMKV::close) the instance.

    If you want to continue using the API, need to [initialize](MMKV::initialize) again.
    */
    pub fn clear_data() {
        mut_mmkv_call!(clear_data);
        MMKV::close();
    }

    /**
    Close the instance to allow MMKV to initialize with different config.

    If you want to continue using the API, need to [initialize](MMKV::initialize) again.
    */
    pub fn close() {
        mut_mmkv_call!(close);
        logger::reset();
    }

    /**
    Set a custom logger for MMKV, MMKV will redirect the inner logs to this logger.

    The default impl of Logger is like this:
    ```
    use mmkv::Logger;

    #[derive(Debug)]
    struct DefaultLogger;

    impl Logger for DefaultLogger {
        fn verbose(&self, log_str: String) {
            println!("{log_str}");
        }

        fn info(&self, log_str: String) {
            println!("{log_str}");
        }

        fn debug(&self, log_str: String) {
            println!("{log_str}");
        }

        fn warn(&self, log_str: String) {
            println!("{log_str}");
        }

        fn error(&self, log_str: String) {
            println!("{log_str}");
        }
    }
    ```
    */
    pub fn set_logger(log_impl: Box<dyn crate::Logger>) {
        logger::set_logger(log_impl);
    }

    /**
    Set log level to mmkv:

    - [LogLevel::Off], no log,
    - [LogLevel::Error]: only display Error logs,
    - [LogLevel::Warn]: display Error and Warn,
    - [LogLevel::Info]: display Error, Warn and Info,
    - [LogLevel::Debug]: display Error, Warn, Info and Debug,
    - [LogLevel::Verbose]: display all logs.

    The default log level is [LogLevel::Verbose].
    */
    pub fn set_log_level(level: LogLevel) {
        logger::set_log_level(level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_instance() {
        let _ = fs::remove_file("mini_mmkv");
        let _ = fs::remove_file("mini_mmkv.meta");
        MMKV::initialize(
            ".",
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        MMKV::initialize(
            ".",
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        MMKV::put_i32("first", 1).unwrap();
        MMKV::put_i32("second", 2).unwrap();
        assert_eq!(MMKV::get_i32("first"), Ok(1));
        assert!(MMKV::get_str("first").is_err());
        assert!(MMKV::get_bool("first").is_err());
        assert_eq!(MMKV::get_i32("second"), Ok(2));
        assert!(MMKV::get_i32("third").is_err());
        MMKV::put_i32("third", 3).unwrap();
        assert_eq!(MMKV::get_i32("third"), Ok(3));
        MMKV::put_str("fourth", "four").unwrap();
        assert_eq!(MMKV::get_str("fourth"), Ok("four".to_string()));
        MMKV::put_str("first", "one").unwrap();
        assert!(MMKV::get_i32("first").is_err());
        assert_eq!(MMKV::get_str("first"), Ok("one".to_string()));
        MMKV::put_bool("second", false).unwrap();
        assert!(MMKV::get_str("second").is_err());
        assert_eq!(MMKV::get_bool("second"), Ok(false));

        MMKV::put_i64("i64", 2).unwrap();
        assert_eq!(MMKV::get_i64("i64"), Ok(2));

        MMKV::put_f32("f32", 2.2).unwrap();
        assert_eq!(MMKV::get_f32("f32"), Ok(2.2));

        MMKV::put_f64("f64", 2.22).unwrap();
        assert_eq!(MMKV::get_f64("f64"), Ok(2.22));

        MMKV::put_byte_array("byte_array", vec![1, 2, 3].as_slice()).unwrap();
        assert_eq!(MMKV::get_byte_array("byte_array"), Ok(vec![1, 2, 3]));

        MMKV::put_i32_array("i32_array", vec![1, 2, 3].as_slice()).unwrap();
        assert_eq!(MMKV::get_i32_array("i32_array"), Ok(vec![1, 2, 3]));

        MMKV::put_i64_array("i64_array", vec![1, 2, 3].as_slice()).unwrap();
        assert_eq!(MMKV::get_i64_array("i64_array"), Ok(vec![1, 2, 3]));

        MMKV::put_f32_array("f32_array", vec![1.1, 2.2, 3.3].as_slice()).unwrap();
        assert_eq!(MMKV::get_f32_array("f32_array"), Ok(vec![1.1, 2.2, 3.3]));

        MMKV::put_f64_array("f64_array", vec![1.1, 2.2, 3.3].as_slice()).unwrap();
        assert_eq!(MMKV::get_f64_array("f64_array"), Ok(vec![1.1, 2.2, 3.3]));

        MMKV::close();
        MMKV::initialize(
            ".",
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        assert_eq!(MMKV::get_str("first"), Ok("one".to_string()));
        MMKV::clear_data();
        assert!(!Path::new("./mini_mmkv").exists());
        assert!(!Path::new("./mini_mmkv.meta").exists());
    }
}
