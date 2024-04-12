use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock, Weak};

use once_cell::sync::Lazy;

use crate::core::buffer::Buffer;
use crate::core::config::Config;
use crate::core::mmkv_impl::MmkvImpl;
use crate::log::logger;
use crate::Error::LockError;
use crate::{LogLevel, Result};

const LOG_TAG: &str = "MMKV:Core";
const DEFAULT_FILE_NAME: &str = "mini_mmkv";

fn page_size() -> usize {
    static PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);

    match PAGE_SIZE.load(Ordering::Relaxed) {
        0 => {
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };

            PAGE_SIZE.store(page_size, Ordering::Relaxed);

            page_size
        }
        page_size => page_size,
    }
}

static INSTANCE_MAP: Lazy<RwLock<HashMap<String, Weak<RwLock<MmkvImpl>>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

pub struct MMKV {
    path: String,
    #[cfg(feature = "encryption")]
    key: String,
    mmkv_impl: Arc<RwLock<MmkvImpl>>,
}

impl Drop for MMKV {
    fn drop(&mut self) {
        let mut map = INSTANCE_MAP.write().unwrap();
        if Arc::strong_count(&self.mmkv_impl) == 1 {
            map.remove(&self.path);
        }
        debug!(
            LOG_TAG,
            "drop MMKV, remain ref count {}",
            Arc::strong_count(&self.mmkv_impl) - 1
        );
    }
}

macro_rules! mmkv_get {
    ($self:ident, $key:expr, $decode:ident) => {
        match $self.mmkv_impl.read() {
            Ok(mmkv) => mmkv.get($key)?.$decode(),
            Err(e) => Err(LockError(e.to_string())),
        }
    };
}

macro_rules! mmkv_put {
    ($self:ident, $key:expr, $value:expr) => {
        match $self.mmkv_impl.write() {
            Ok(mut mmkv) => mmkv.put($key, $value),
            Err(e) => Err(LockError(e.to_string())),
        }
    };
}

macro_rules! mmkv_delete {
    ($self:ident, $key:expr) => {
        match $self.mmkv_impl.write() {
            Ok(mut mmkv) => mmkv.delete($key),
            Err(e) => Err(LockError(e.to_string())),
        }
    };
}

impl MMKV {
    /**
    Initialize the MMKV instance with a writeable directory,
    absolute or relative paths are acceptable.

    All API calls(except [set_logger](MMKV::set_logger), [set_log_level](MMKV::set_log_level))
    before initialization will panic.

    Calling [initialize](MMKV::initialize) multiple times is allowed,
    the old instance will be closed (see [close](MMKV::close)), the last call will take over.

    If enabled feature "encryption", additional param `key` is required,
    the key should be a hexadecimal string of length 16, for example:

    `88C51C536176AD8A8EE4A06F62EE897E`
    */
    pub fn new(dir: &str, #[cfg(feature = "encryption")] key: &str) -> Self {
        let instance_map = INSTANCE_MAP.read().unwrap();
        if let Some(mmkv) = instance_map.get(dir).and_then(|mmkv| mmkv.upgrade()) {
            debug!(LOG_TAG, "new MMKV from existing instance");
            return MMKV {
                path: dir.to_string(),
                #[cfg(feature = "encryption")]
                key: key.to_string(),
                mmkv_impl: mmkv,
            };
        }
        drop(instance_map);

        let mut instance_map = INSTANCE_MAP.write().unwrap();
        // Double check if other thread completed init
        if let Some(mmkv) = instance_map.get(dir).and_then(|mmkv| mmkv.upgrade()) {
            debug!(
                LOG_TAG,
                "new MMKV from existing instance after double check"
            );
            return MMKV {
                path: dir.to_string(),
                #[cfg(feature = "encryption")]
                key: key.to_string(),
                mmkv_impl: mmkv.clone(),
            };
        }
        // Init a new instance
        let file_path = MMKV::resolve_file_path(dir);
        let config = Config::new(file_path.as_path(), page_size() as u64);
        let mmkv_impl = Arc::new(RwLock::new(MmkvImpl::new(
            config,
            #[cfg(feature = "encryption")]
            key,
        )));
        instance_map.insert(dir.to_string(), Arc::downgrade(&mmkv_impl));
        MMKV {
            path: dir.to_string(),
            #[cfg(feature = "encryption")]
            key: key.to_string(),
            mmkv_impl,
        }
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

    pub fn put_str(&self, key: &str, value: &str) -> Result<()> {
        mmkv_put!(self, key, Buffer::from_str(key, value))
    }

    pub fn get_str(&self, key: &str) -> Result<String> {
        mmkv_get!(self, key, decode_str)
    }

    pub fn put_i32(&self, key: &str, value: i32) -> Result<()> {
        mmkv_put!(self, key, Buffer::from_i32(key, value))
    }

    pub fn get_i32(&self, key: &str) -> Result<i32> {
        mmkv_get!(self, key, decode_i32)
    }

    pub fn put_bool(&self, key: &str, value: bool) -> Result<()> {
        mmkv_put!(self, key, Buffer::from_bool(key, value))
    }

    pub fn get_bool(&self, key: &str) -> Result<bool> {
        mmkv_get!(self, key, decode_bool)
    }

    pub fn put_i64(&self, key: &str, value: i64) -> Result<()> {
        mmkv_put!(self, key, Buffer::from_i64(key, value))
    }

    pub fn get_i64(&self, key: &str) -> Result<i64> {
        mmkv_get!(self, key, decode_i64)
    }

    pub fn put_f32(&self, key: &str, value: f32) -> Result<()> {
        mmkv_put!(self, key, Buffer::from_f32(key, value))
    }

    pub fn get_f32(&self, key: &str) -> Result<f32> {
        mmkv_get!(self, key, decode_f32)
    }

    pub fn put_f64(&self, key: &str, value: f64) -> Result<()> {
        mmkv_put!(self, key, Buffer::from_f64(key, value))
    }

    pub fn get_f64(&self, key: &str) -> Result<f64> {
        mmkv_get!(self, key, decode_f64)
    }

    pub fn put_byte_array(&self, key: &str, value: &[u8]) -> Result<()> {
        mmkv_put!(self, key, Buffer::from_byte_array(key, value))
    }

    pub fn get_byte_array(&self, key: &str) -> Result<Vec<u8>> {
        mmkv_get!(self, key, decode_byte_array)
    }

    pub fn put_i32_array(&self, key: &str, value: &[i32]) -> Result<()> {
        mmkv_put!(self, key, Buffer::from_i32_array(key, value))
    }

    pub fn get_i32_array(&self, key: &str) -> Result<Vec<i32>> {
        mmkv_get!(self, key, decode_i32_array)
    }

    pub fn put_i64_array(&self, key: &str, value: &[i64]) -> Result<()> {
        mmkv_put!(self, key, Buffer::from_i64_array(key, value))
    }

    pub fn get_i64_array(&self, key: &str) -> Result<Vec<i64>> {
        mmkv_get!(self, key, decode_i64_array)
    }

    pub fn put_f32_array(&self, key: &str, value: &[f32]) -> Result<()> {
        mmkv_put!(self, key, Buffer::from_f32_array(key, value))
    }

    pub fn get_f32_array(&self, key: &str) -> Result<Vec<f32>> {
        mmkv_get!(self, key, decode_f32_array)
    }

    pub fn put_f64_array(&self, key: &str, value: &[f64]) -> Result<()> {
        mmkv_put!(self, key, Buffer::from_f64_array(key, value))
    }

    pub fn get_f64_array(&self, key: &str) -> Result<Vec<f64>> {
        mmkv_get!(self, key, decode_f64_array)
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        mmkv_delete!(self, key)
    }

    /**
    Clear all data.
    */
    pub fn clear_data(&self) {
        let mut mmkv_impl = self.mmkv_impl.write().unwrap();
        mmkv_impl.clear_data();
        let file_path = MMKV::resolve_file_path(&self.path);
        let config = Config::new(file_path.as_path(), page_size() as u64);
        *mmkv_impl = MmkvImpl::new(
            config,
            #[cfg(feature = "encryption")]
            &self.key,
        );
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
        logger::set_logger(Some(log_impl));
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
    use std::fs;

    use crate::Error::KeyNotFound;

    use super::*;

    #[test]
    #[allow(unused_assignments)]
    fn test_instance() {
        let _ = fs::remove_file("mini_mmkv");
        let _ = fs::remove_file("mini_mmkv.meta");
        let mut mmkv = MMKV::new(
            ".",
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        debug!(LOG_TAG, "---------------");
        mmkv = MMKV::new(
            ".",
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        mmkv.put_i32("first", 1).unwrap();
        mmkv.put_i32("second", 2).unwrap();
        assert_eq!(mmkv.get_i32("first"), Ok(1));
        assert!(mmkv.get_str("first").is_err());
        assert!(mmkv.get_bool("first").is_err());
        assert_eq!(mmkv.get_i32("second"), Ok(2));
        assert!(mmkv.get_i32("third").is_err());
        mmkv.put_i32("third", 3).unwrap();
        assert_eq!(mmkv.get_i32("third"), Ok(3));
        mmkv.put_str("fourth", "four").unwrap();
        assert_eq!(mmkv.get_str("fourth"), Ok("four".to_string()));
        mmkv.put_str("first", "one").unwrap();
        assert!(mmkv.get_i32("first").is_err());
        assert_eq!(mmkv.get_str("first"), Ok("one".to_string()));
        mmkv.put_bool("second", false).unwrap();
        assert!(mmkv.get_str("second").is_err());
        assert_eq!(mmkv.get_bool("second"), Ok(false));

        mmkv.put_i64("i64", 2).unwrap();
        assert_eq!(mmkv.get_i64("i64"), Ok(2));

        mmkv.put_f32("f32", 2.2).unwrap();
        assert_eq!(mmkv.get_f32("f32"), Ok(2.2));

        mmkv.put_f64("f64", 2.22).unwrap();
        assert_eq!(mmkv.get_f64("f64"), Ok(2.22));

        mmkv.put_byte_array("byte_array", vec![1, 2, 3].as_slice())
            .unwrap();
        assert_eq!(mmkv.get_byte_array("byte_array"), Ok(vec![1, 2, 3]));

        mmkv.put_i32_array("i32_array", vec![1, 2, 3].as_slice())
            .unwrap();
        assert_eq!(mmkv.get_i32_array("i32_array"), Ok(vec![1, 2, 3]));

        mmkv.put_i64_array("i64_array", vec![1, 2, 3].as_slice())
            .unwrap();
        assert_eq!(mmkv.get_i64_array("i64_array"), Ok(vec![1, 2, 3]));

        mmkv.put_f32_array("f32_array", vec![1.1, 2.2, 3.3].as_slice())
            .unwrap();
        assert_eq!(mmkv.get_f32_array("f32_array"), Ok(vec![1.1, 2.2, 3.3]));

        mmkv.put_f64_array("f64_array", vec![1.1, 2.2, 3.3].as_slice())
            .unwrap();
        assert_eq!(mmkv.get_f64_array("f64_array"), Ok(vec![1.1, 2.2, 3.3]));

        mmkv.delete("second").unwrap();
        assert_eq!(mmkv.get_i32("second"), Err(KeyNotFound));
        drop(mmkv);
        debug!(LOG_TAG, "---------------");

        mmkv = MMKV::new(
            ".",
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        assert_eq!(mmkv.get_str("first"), Ok("one".to_string()));
        assert_eq!(mmkv.get_i32("second"), Err(KeyNotFound));
        mmkv.clear_data();
        let _ = fs::remove_file("mini_mmkv");
        let _ = fs::remove_file("mini_mmkv.meta");
    }
}
