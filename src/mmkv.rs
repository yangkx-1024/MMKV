use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock, Weak};

use once_cell::sync::Lazy;

use crate::Error::{IOError, LockError};
use crate::core::buffer::{Buffer, FromBytes, ProvideTypeToken, ToBytes};
use crate::core::config::Config;
use crate::core::mmkv_impl::MmkvImpl;
use crate::log::logger;
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

static INSTANCE_MAP: Lazy<RwLock<HashMap<PathBuf, Weak<RwLock<MmkvImpl>>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

pub struct MMKV {
    path: PathBuf,
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
    pub fn new(dir: &str, #[cfg(feature = "encryption")] key: &str) -> Result<Self> {
        let dir = MMKV::resolve_dir_path(dir)?;
        let instance_map = INSTANCE_MAP.read().unwrap();
        if let Some(mmkv) = instance_map.get(&dir).and_then(|mmkv| mmkv.upgrade()) {
            debug!(LOG_TAG, "new MMKV from existing instance");
            return Ok(MMKV {
                path: dir.clone(),
                #[cfg(feature = "encryption")]
                key: key.to_string(),
                mmkv_impl: mmkv,
            });
        }
        drop(instance_map);

        let mut instance_map = INSTANCE_MAP.write().unwrap();
        // Double check if other thread completed init
        if let Some(mmkv) = instance_map.get(&dir).and_then(|mmkv| mmkv.upgrade()) {
            debug!(
                LOG_TAG,
                "new MMKV from existing instance after double check"
            );
            return Ok(MMKV {
                path: dir.clone(),
                #[cfg(feature = "encryption")]
                key: key.to_string(),
                mmkv_impl: mmkv.clone(),
            });
        }
        // Init a new instance
        let file_path = MMKV::resolve_file_path(&dir);
        let config = Config::new(file_path.as_path(), page_size() as u64)?;
        let mmkv_impl = Arc::new(RwLock::new(MmkvImpl::new(
            config,
            #[cfg(feature = "encryption")]
            key,
        )?));
        instance_map.insert(dir.clone(), Arc::downgrade(&mmkv_impl));
        Ok(MMKV {
            path: dir,
            #[cfg(feature = "encryption")]
            key: key.to_string(),
            mmkv_impl,
        })
    }

    fn resolve_dir_path(dir: &str) -> Result<PathBuf> {
        let path = Path::new(dir);
        if !path.is_dir() {
            return Err(IOError(format!("path {dir} is not dir")));
        }
        let canonical_dir = fs::canonicalize(path)
            .map_err(|e| IOError(format!("failed to canonicalize dir {dir}: {e}")))?;
        let metadata = canonical_dir
            .metadata()
            .map_err(|e| IOError(format!("failed to get attr of dir {dir}: {e}")))?;
        if metadata.permissions().readonly() {
            return Err(IOError(format!("path {dir} is readonly")));
        }
        Ok(canonical_dir)
    }

    fn resolve_file_path(dir: &Path) -> PathBuf {
        dir.join(DEFAULT_FILE_NAME)
    }

    /**
    Types must implement [ProvideTypeToken] and [ToBytes] to be persisted in MMKV.

    `put` returns once the record has been written to the memory-mapped file, so
    `Ok(())` means the value is visible to every reader of this process and lives in
    the OS page cache, where it survives a crash of this process. No `fsync` is issued
    per write. Any encode or IO failure is returned as `Err` and leaves the previous
    value in place, both in memory and on disk.

    If you want to persist custom struct to MMKV,
    your struct must implement [ToBytes] trait which serialize type to bytes,
    and [FromBytes] trait which deserialize type from bytes.
    For example:
    ```
    use mmkv::{FromBytes, MMKV, ProvideTypeToken, ToBytes, TypeToken};
    #[derive(Clone, PartialEq, Eq, Debug, Hash)]
    struct MyStruct {
        int_value: i32,
        str_value: String,
    }

    impl ProvideTypeToken for MyStruct {
        fn type_token() -> TypeToken {
            TypeToken::new(101)
        }
    }

    impl ToBytes for MyStruct {
        fn to_bytes(&self) -> Vec<u8> {
            let mut vec = vec![];
            vec.extend(self.int_value.to_be_bytes());
            vec.extend(self.str_value.as_bytes().to_vec());
            vec
        }
    }

    impl FromBytes for MyStruct {
        fn from_bytes(bytes: &[u8]) -> mmkv::Result<Self> {
            let int_len = size_of::<i32>();
            let int_val = i32::from_be_bytes(bytes[0..int_len].try_into().unwrap());
            let str_val = String::from_utf8(bytes[int_len..].try_into().unwrap()).unwrap();
            Ok(MyStruct {
                int_value: int_val,
                str_value: str_val
            })
        }
    }

    let dir = std::env::temp_dir().join("mmkv_doc_put");
    std::fs::create_dir_all(&dir).unwrap();
    let mmkv = MMKV::new(dir.to_str().unwrap(), #[cfg(feature = "encryption")] "88C51C536176AD8A8EE4A06F62EE897E").unwrap();
    let my_struct = MyStruct {
        int_value: 1,
        str_value: "abc".to_string(),
    };
    mmkv.put("my_struct", &my_struct).unwrap();
    let copy: MyStruct = mmkv.get("my_struct").unwrap();
    assert_eq!(my_struct, copy)
    ```
    */
    pub fn put<T: ProvideTypeToken + ToBytes>(&self, key: &str, value: T) -> Result<()> {
        match self.mmkv_impl.write() {
            Ok(mut mmkv) => mmkv.put(key, Buffer::new(key, value)),
            Err(e) => Err(LockError(e.to_string())),
        }
    }

    /// See [MMKV::put]
    pub fn get<T: ProvideTypeToken + FromBytes>(&self, key: &str) -> Result<T> {
        match self.mmkv_impl.read() {
            Ok(mmkv) => mmkv.get::<T>(key),
            Err(e) => Err(LockError(e.to_string())),
        }
    }

    /**
    Delete `key`. Returns once the tombstone has been written to the memory-mapped
    file; on failure the key keeps its previous value. Deleting a missing key is `Ok`.
    */
    pub fn delete(&self, key: &str) -> Result<()> {
        match self.mmkv_impl.write() {
            Ok(mut mmkv) => mmkv.delete(key),
            Err(e) => Err(LockError(e.to_string())),
        }
    }

    /**
    Clear all data.
    */
    pub fn clear_data(&self) -> Result<()> {
        let mut mmkv_impl = self
            .mmkv_impl
            .write()
            .map_err(|e| LockError(e.to_string()))?;
        mmkv_impl.clear_data()?;
        let file_path = MMKV::resolve_file_path(&self.path);
        let config = Config::new(file_path.as_path(), page_size() as u64)?;
        *mmkv_impl = MmkvImpl::new(
            config,
            #[cfg(feature = "encryption")]
            &self.key,
        )?;
        Ok(())
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

/// Unit tests for the instance-cache internals of [MMKV], which need the private
/// `mmkv_impl` field and `INSTANCE_MAP`. Everything observable through the public API is
/// tested in `tests/` instead.
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tempfile::tempdir;

    #[cfg(feature = "encryption")]
    use crate::core::test_support::TEST_KEY;

    use super::*;

    fn open(dir: &Path) -> MMKV {
        MMKV::new(
            dir.to_str().unwrap(),
            #[cfg(feature = "encryption")]
            TEST_KEY,
        )
        .unwrap()
    }

    #[test]
    fn the_instance_cache_is_keyed_by_the_canonical_dir() {
        let temp = tempdir().unwrap();
        let dir = temp.path();

        let dir_with_trailing_slash = format!("{}/", dir.display());
        let mmkv = open(dir);
        let mmkv_same_dir = MMKV::new(
            &dir_with_trailing_slash,
            #[cfg(feature = "encryption")]
            TEST_KEY,
        )
        .unwrap();

        assert!(Arc::ptr_eq(&mmkv.mmkv_impl, &mmkv_same_dir.mmkv_impl));

        mmkv.clear_data().unwrap();
        drop(mmkv_same_dir);
        drop(mmkv);
    }

    #[test]
    fn dropping_the_last_handle_evicts_the_dir_from_the_cache() {
        let temp = tempdir().unwrap();
        let dir = temp.path();
        let canonical = fs::canonicalize(dir).unwrap();

        let mmkv = open(dir);
        let second_handle = open(dir);
        assert!(INSTANCE_MAP.read().unwrap().contains_key(&canonical));

        drop(second_handle);
        assert!(
            INSTANCE_MAP.read().unwrap().contains_key(&canonical),
            "a live handle must keep the entry"
        );

        drop(mmkv);
        assert!(
            !INSTANCE_MAP.read().unwrap().contains_key(&canonical),
            "the last handle must evict the entry"
        );

        // A later open creates a fresh entry rather than resurrecting a dead Weak.
        let reopened = open(dir);
        assert!(
            INSTANCE_MAP
                .read()
                .unwrap()
                .get(&canonical)
                .and_then(|weak| weak.upgrade())
                .is_some()
        );
        reopened.clear_data().unwrap();
    }

    #[test]
    fn new_rejects_a_path_that_is_not_a_writable_dir() {
        let temp = tempdir().unwrap();
        let file = temp.path().join("not_a_dir");
        fs::write(&file, b"x").unwrap();
        let missing = temp.path().join("missing_dir");

        for path in [&file, &missing] {
            let result = MMKV::new(
                path.to_str().unwrap(),
                #[cfg(feature = "encryption")]
                TEST_KEY,
            );
            assert!(
                matches!(result, Err(IOError(_))),
                "{} must be rejected",
                path.display()
            );
        }
    }
}
