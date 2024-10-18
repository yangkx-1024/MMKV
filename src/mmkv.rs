use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock, Weak};

use once_cell::sync::Lazy;

use crate::core::buffer::{Buffer, FromBuffer, ToBuffer};
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

impl MMKV {
    /**
    Initialize the MMKV instance with a writeable directory,
    absolute or relative paths are acceptable.

    Calling [new](MMKV::new) multiple times with same parameter `dir` will get different MMKV
    instances share the same mmap, it's safe to call get or put concurrently on these instances.

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

    pub fn put<T: ToBuffer>(&self, key: &str, value: T) -> Result<()> {
        match self.mmkv_impl.write() {
            Ok(mut mmkv) => mmkv.put(key, Buffer::encode(key, value)),
            Err(e) => Err(LockError(e.to_string())),
        }
    }

    pub fn get<T: FromBuffer>(&self, key: &str) -> Result<T> {
        match self.mmkv_impl.read() {
            Ok(mmkv) => mmkv.get(key)?.decode(),
            Err(e) => Err(LockError(e.to_string())),
        }
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        match self.mmkv_impl.write() {
            Ok(mut mmkv) => mmkv.delete(key),
            Err(e) => Err(LockError(e.to_string())),
        }
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
        mmkv.put("first", 1i32).unwrap();
        mmkv.put("second", 2i32).unwrap();
        assert_eq!(mmkv.get("first"), Ok(1));
        assert!(mmkv.get::<String>("first").is_err());
        assert!(mmkv.get::<bool>("first").is_err());
        assert_eq!(mmkv.get("second"), Ok(2));
        assert!(mmkv.get::<i32>("third").is_err());
        mmkv.put("third", 3).unwrap();
        assert_eq!(mmkv.get("third"), Ok(3));
        mmkv.put("fourth", "four").unwrap();
        assert_eq!(mmkv.get("fourth"), Ok("four".to_string()));
        mmkv.put("first", "one").unwrap();
        assert!(mmkv.get::<i32>("first").is_err());
        assert_eq!(mmkv.get("first"), Ok("one".to_string()));
        mmkv.put("second", false).unwrap();
        assert!(mmkv.get::<String>("second").is_err());
        assert_eq!(mmkv.get("second"), Ok(false));

        mmkv.put("i64", 2i64).unwrap();
        assert_eq!(mmkv.get::<i64>("i64"), Ok(2));

        mmkv.put("f32", 2.2f32).unwrap();
        assert_eq!(mmkv.get::<f32>("f32"), Ok(2.2));

        mmkv.put("f64", 2.22f64).unwrap();
        assert_eq!(mmkv.get::<f64>("f64"), Ok(2.22));

        mmkv.put("byte_array", vec![1u8, 2, 3].as_slice()).unwrap();
        assert_eq!(mmkv.get::<Vec<u8>>("byte_array"), Ok(vec![1, 2, 3]));

        mmkv.put("i32_array", vec![1i32, 2, 3].as_slice()).unwrap();
        assert_eq!(mmkv.get("i32_array"), Ok(vec![1, 2, 3]));

        mmkv.put("i64_array", vec![1i64, 2, 3].as_slice()).unwrap();
        assert_eq!(mmkv.get("i64_array"), Ok(vec![1i64, 2, 3]));

        mmkv.put("f32_array", vec![1.1f32, 2.2, 3.3].as_slice())
            .unwrap();
        assert_eq!(mmkv.get::<Vec<f32>>("f32_array"), Ok(vec![1.1, 2.2, 3.3]));

        mmkv.put("f64_array", vec![1.1f64, 2.2, 3.3].as_slice())
            .unwrap();
        assert_eq!(mmkv.get::<Vec<f64>>("f64_array"), Ok(vec![1.1, 2.2, 3.3]));

        mmkv.delete("second").unwrap();
        assert_eq!(mmkv.get::<i32>("second"), Err(KeyNotFound));
        drop(mmkv);
        debug!(LOG_TAG, "---------------");

        mmkv = MMKV::new(
            ".",
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        assert_eq!(mmkv.get("first"), Ok("one".to_string()));
        assert_eq!(mmkv.get::<i32>("second"), Err(KeyNotFound));
        mmkv.clear_data();
        let _ = fs::remove_file("mini_mmkv");
        let _ = fs::remove_file("mini_mmkv.meta");
    }
}
