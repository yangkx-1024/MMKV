use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::core::buffer::Buffer;
use crate::core::mmkv_impl::MmkvImpl;
use crate::log::logger;
use crate::{LogLevel, Result};

const LOG_TAG: &str = "MMKV";
const DEFAULT_FILE_NAME: &str = "mini_mmkv";
const PAGE_SIZE: u64 = 4 * 1024; // 4KB is the default Linux page size

static MMKV_IMPL: AtomicPtr<MmkvImpl> = AtomicPtr::new(std::ptr::null_mut());

macro_rules! mmkv {
    () => {{
        let ptr = MMKV_IMPL.load(Ordering::Acquire);
        unsafe { ptr.as_ref().unwrap() }
    }};
}

macro_rules! mut_mmkv {
    () => {{
        let ptr = MMKV_IMPL.load(Ordering::Acquire);
        unsafe { ptr.as_mut().unwrap() }
    }};
}

pub struct MMKV;

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
        MMKV::drop_instance();
        let file_path = MMKV::resolve_file_path(dir);
        let mmkv_impl = MmkvImpl::new(
            file_path.as_path(),
            PAGE_SIZE,
            #[cfg(feature = "encryption")]
            key,
        );
        let raw_ptr = Box::into_raw(Box::new(mmkv_impl));
        MMKV_IMPL.swap(raw_ptr, Ordering::Release);
        verbose!(LOG_TAG, "instance initialized");
        MMKV::dump();
    }

    fn resolve_file_path(dir: &str) -> PathBuf {
        let path = Path::new(dir);
        if !path.is_dir() {
            panic!("path {}, is not dir", dir);
        }
        let metadata = path
            .metadata()
            .expect(format!("failed to get attr of dir {}", dir).as_str());
        if metadata.permissions().readonly() {
            panic!("path {}, is readonly", dir);
        }
        path.join(DEFAULT_FILE_NAME)
    }

    pub fn put_str(key: &str, value: &str) -> Result<()> {
        mut_mmkv!().put(key, Buffer::from_str(key, value))
    }

    pub fn get_str(key: &str) -> Result<&str> {
        mmkv!().get(key)?.decode_str()
    }

    pub fn put_i32(key: &str, value: i32) -> Result<()> {
        mut_mmkv!().put(key, Buffer::from_i32(key, value))
    }

    pub fn get_i32(key: &str) -> Result<i32> {
        mmkv!().get(key)?.decode_i32()
    }

    pub fn put_bool(key: &str, value: bool) -> Result<()> {
        mut_mmkv!().put(key, Buffer::from_bool(key, value))
    }

    pub fn get_bool(key: &str) -> Result<bool> {
        mmkv!().get(key)?.decode_bool()
    }

    pub fn put_i64(key: &str, value: i64) -> Result<()> {
        mut_mmkv!().put(key, Buffer::from_i64(key, value))
    }

    pub fn get_i64(key: &str) -> Result<i64> {
        mmkv!().get(key)?.decode_i64()
    }

    pub fn put_f32(key: &str, value: f32) -> Result<()> {
        mut_mmkv!().put(key, Buffer::from_f32(key, value))
    }

    pub fn get_f32(key: &str) -> Result<f32> {
        mmkv!().get(key)?.decode_f32()
    }

    pub fn put_f64(key: &str, value: f64) -> Result<()> {
        mut_mmkv!().put(key, Buffer::from_f64(key, value))
    }

    pub fn get_f64(key: &str) -> Result<f64> {
        mmkv!().get(key)?.decode_f64()
    }

    pub fn put_byte_array(key: &str, value: &[u8]) -> Result<()> {
        mut_mmkv!().put(key, Buffer::from_byte_array(key, value))
    }

    pub fn get_byte_array(key: &str) -> Result<Vec<u8>> {
        mmkv!().get(key)?.decode_byte_array()
    }

    pub fn put_i32_array(key: &str, value: &[i32]) -> Result<()> {
        mut_mmkv!().put(key, Buffer::from_i32_array(key, value))
    }

    pub fn get_i32_array(key: &str) -> Result<Vec<i32>> {
        mmkv!().get(key)?.decode_i32_array()
    }

    pub fn put_i64_array(key: &str, value: &[i64]) -> Result<()> {
        mut_mmkv!().put(key, Buffer::from_i64_array(key, value))
    }

    pub fn get_i64_array(key: &str) -> Result<Vec<i64>> {
        mmkv!().get(key)?.decode_i64_array()
    }

    pub fn put_f32_array(key: &str, value: &[f32]) -> Result<()> {
        mut_mmkv!().put(key, Buffer::from_f32_array(key, value))
    }

    pub fn get_f32_array(key: &str) -> Result<Vec<f32>> {
        mmkv!().get(key)?.decode_f32_array()
    }

    pub fn put_f64_array(key: &str, value: &[f64]) -> Result<()> {
        mut_mmkv!().put(key, Buffer::from_f64_array(key, value))
    }

    pub fn get_f64_array(key: &str) -> Result<Vec<f64>> {
        mmkv!().get(key)?.decode_f64_array()
    }

    /**
    Clear all data and [close](MMKV::close) the instance.

    If you want to continue using the API, need to [initialize](MMKV::initialize) again.
    */
    pub fn clear_data() {
        mut_mmkv!().clear_data();
        MMKV::close();
    }

    /**
    Close the instance to allow MMKV to initialize with different config.

    If you want to continue using the API, need to [initialize](MMKV::initialize) again.
    */
    pub fn close() {
        MMKV::drop_instance();
        logger::reset();
    }

    fn drop_instance() {
        let p = MMKV_IMPL.swap(std::ptr::null_mut(), Ordering::Release);
        if p != std::ptr::null_mut() {
            unsafe {
                drop(Box::from_raw(p));
                verbose!(LOG_TAG, "instance closed");
            }
        }
    }

    /**
    Dump the current state of MMKV, the result looks like this:

    `MMKV { file_size: 1024, key_count: 4, content_len: 107 }`
     */
    pub fn dump() -> String {
        let str = mmkv!().to_string();
        debug!(LOG_TAG, "dump state {}", &str);
        str
    }

    /**
    Set a custom logger for MMKV, MMKV will redirect the inner logs to this logger.

    The default impl of Logger is like this:
    ```
    use mmkv::Logger;

    #[derive(Debug)]
    struct DefaultLogger;

    impl Logger for DefaultLogger {
        fn verbose(&self, log_str: &str) {
            println!("{log_str}");
        }

        fn info(&self, log_str: &str) {
            println!("{log_str}");
        }

        fn debug(&self, log_str: &str) {
            println!("{log_str}");
        }

        fn warn(&self, log_str: &str) {
            println!("{log_str}");
        }

        fn error(&self, log_str: &str) {
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
    - [LogLevel::Debug]: display all logs.

    The default log level is [LogLevel::Debug].
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
        assert_eq!(MMKV::get_str("first").is_err(), true);
        assert_eq!(MMKV::get_bool("first").is_err(), true);
        assert_eq!(MMKV::get_i32("second"), Ok(2));
        assert_eq!(MMKV::get_i32("third").is_err(), true);
        MMKV::put_i32("third", 3).unwrap();
        assert_eq!(MMKV::get_i32("third"), Ok(3));
        MMKV::put_str("fourth", "four").unwrap();
        assert_eq!(MMKV::get_str("fourth"), Ok("four"));
        MMKV::put_str("first", "one").unwrap();
        assert_eq!(MMKV::get_i32("first").is_err(), true);
        assert_eq!(MMKV::get_str("first"), Ok("one"));
        MMKV::put_bool("second", false).unwrap();
        assert_eq!(MMKV::get_str("second").is_err(), true);
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
        assert_eq!(MMKV_IMPL.load(Ordering::Acquire), std::ptr::null_mut());
        MMKV::initialize(
            ".",
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        assert_eq!(MMKV::get_str("first"), Ok("one"));
        MMKV::clear_data();
        assert_eq!(Path::new("./mini_mmkv").exists(), false);
        assert_eq!(Path::new("./mini_mmkv.meta").exists(), false);
        assert_eq!(MMKV_IMPL.load(Ordering::Acquire), std::ptr::null_mut());
    }
}
