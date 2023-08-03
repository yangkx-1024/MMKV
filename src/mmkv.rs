use std::path::Path;
use std::sync::OnceLock;

use crate::core::buffer::{Buffer, Decoder};
use crate::core::kv_store::KvStore;

const _DEFAULT_FILE_NAME: &str = "mini_mmkv";
const _PAGE_SIZE: u64 = 1024;

static mut _STORE: OnceLock<KvStore> = OnceLock::new();

/**
Rust version of MMKV.

Examples:

```
use mmkv::MMKV;

MMKV::initialize(".");
MMKV::put_i32("key1", 1);
assert_eq!(MMKV::get_i32("key1"), Some(1));
```
 */
pub struct MMKV;

impl MMKV {
    /**
    Initialize the MMKV instance with a writeable directory,
    absolute or relative paths are acceptable.

    All API calls before initialization will panic.

    There will only be one MMKV instance globally,
    calling initialize multiple times will also cause panic.
     */
    pub fn initialize(dir: &str) {
        let path = Path::new(dir);
        if !path.is_dir() {
            panic!("path {}, is not dir", dir);
        }
        let metadata = path.metadata().expect(format!("failed to get attr of dir {}", dir).as_str());
        if metadata.permissions().readonly() {
            panic!("path {}, is readonly", dir);
        }
        let file_path = path.join(_DEFAULT_FILE_NAME);
        unsafe {
            _STORE.set(
                KvStore::new(file_path.as_path(), _PAGE_SIZE)
            ).expect("initialize more than one time");
        }
    }

    pub fn put_str(key: &str, value: &str) {
        _ensure_mut_store().write(key, Buffer::from_str(key, value));
    }

    pub fn get_str(key: &str) -> Option<&str> {
        _ensure_store().get(key).map(|buffer| {
            buffer.decode_str()
        }).flatten()
    }

    pub fn put_i32(key: &str, value: i32) {
        _ensure_mut_store().write(key, Buffer::from_i32(key, value));
    }

    pub fn get_i32(key: &str) -> Option<i32> {
        _ensure_store().get(key).map(|buffer| {
            buffer.decode_i32()
        }).flatten()
    }

    pub fn put_bool(key: &str, value: bool) {
        _ensure_mut_store().write(key, Buffer::from_bool(key, value));
    }

    pub fn get_bool(key: &str) -> Option<bool> {
        _ensure_store().get(key).map(|buffer| {
            buffer.decode_bool()
        }).flatten()
    }

    /**
    Dump the current state of MMKV, the result looks like this:

    `KvStore { file_size: 1024, key_count: 4, content_len: 107 }`
     */
    pub fn dump() -> String {
        _ensure_store().to_string()
    }
}

fn _ensure_store() -> &'static KvStore {
    unsafe {
        _STORE.get().expect("not initialize")
    }
}

fn _ensure_mut_store() -> &'static mut KvStore {
    unsafe {
        _STORE.get_mut().expect("not initialize")
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::MMKV;

    #[test]
    fn test_put_and_get() {
        MMKV::initialize(".");
        MMKV::put_i32("first", 1);
        MMKV::put_i32("second", 2);
        assert_eq!(MMKV::get_i32("first"), Some(1));
        assert_eq!(MMKV::get_str("first"), None);
        assert_eq!(MMKV::get_bool("first"), None);
        assert_eq!(MMKV::get_i32("second"), Some(2));
        assert_eq!(MMKV::get_i32("third"), None);
        MMKV::put_i32("third", 3);
        assert_eq!(MMKV::get_i32("third"), Some(3));
        MMKV::put_str("fourth", "four");
        assert_eq!(MMKV::get_str("fourth"), Some("four"));
        MMKV::put_str("first", "one");
        assert_eq!(MMKV::get_i32("first"), None);
        assert_eq!(MMKV::get_str("first"), Some("one"));
        MMKV::put_bool("second", false);
        assert_eq!(MMKV::get_str("second"), None);
        assert_eq!(MMKV::get_bool("second"), Some(false));
        println!("{}", MMKV::dump());
        let _ = fs::remove_file("mini_mmkv");
    }
}