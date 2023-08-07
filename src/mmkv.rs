use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::core::buffer::Buffer;
use crate::core::mmkv_impl::MmkvImpl;

const _DEFAULT_FILE_NAME: &str = "mini_mmkv";
const _PAGE_SIZE: u64 = 4 * 1024; // 4KB is the default Linux page size

static mut MMKV_IMPL: OnceLock<MmkvImpl> = OnceLock::new();

macro_rules! kv_store {
    () => {
        {
            unsafe {
                MMKV_IMPL.get().expect("not initialize")
            }
        }
    }
}

macro_rules! mut_kv_store {
    () => {
        {
            unsafe {
                MMKV_IMPL.get_mut().expect("not initialize")
            }
        }
    }
}

/**
Rust version of MMKV.

Examples:

Using directly:
```
use mmkv::MMKV;

MMKV::initialize(".");
MMKV::put_i32("key1", 1);
assert_eq!(MMKV::get_i32("key1"), Some(1));
```

Using with encryption:
```
use mmkv::MMKV;

MMKV::initialize_with_encrypt_key(".", "88C51C536176AD8A8EE4A06F62EE897E");
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
        let file_path = MMKV::resolve_file_path(dir);
        unsafe {
            MMKV_IMPL.set(
                MmkvImpl::new(file_path.as_path(), _PAGE_SIZE)
            ).expect("initialize more than one time");
        }
    }

    /**
    See [initialize](MMKV::initialize)

    Initialize the MMKV instance with a writeable directory,
    and a credential for encryption(enable content encryption).

    The key should be a hexadecimal string of length 16, for example:
    "88C51C536176AD8A8EE4A06F62EE897E".
     */
    pub fn initialize_with_encrypt_key(dir: &str, key: &str) {
        let file_path = MMKV::resolve_file_path(dir);
        unsafe {
            MMKV_IMPL.set(
                MmkvImpl::new_with_encrypt_key(file_path.as_path(), _PAGE_SIZE, key)
            ).expect("initialize more than one time");
        }
    }

    fn resolve_file_path(dir: &str) -> PathBuf {
        let path = Path::new(dir);
        if !path.is_dir() {
            panic!("path {}, is not dir", dir);
        }
        let metadata = path.metadata().expect(format!("failed to get attr of dir {}", dir).as_str());
        if metadata.permissions().readonly() {
            panic!("path {}, is readonly", dir);
        }
        path.join(_DEFAULT_FILE_NAME)
    }

    pub fn put_str(key: &str, value: &str) {
        mut_kv_store!().write(key, Buffer::from_str(key, value));
    }

    pub fn get_str(key: &str) -> Option<&str> {
        kv_store!().get(key).map(|buffer| {
            buffer.decode_str()
        }).flatten()
    }

    pub fn put_i32(key: &str, value: i32) {
        mut_kv_store!().write(key, Buffer::from_i32(key, value));
    }

    pub fn get_i32(key: &str) -> Option<i32> {
        kv_store!().get(key).map(|buffer| {
            buffer.decode_i32()
        }).flatten()
    }

    pub fn put_bool(key: &str, value: bool) {
        mut_kv_store!().write(key, Buffer::from_bool(key, value));
    }

    pub fn get_bool(key: &str) -> Option<bool> {
        kv_store!().get(key).map(|buffer| {
            buffer.decode_bool()
        }).flatten()
    }

    /**
    Dump the current state of MMKV, the result looks like this:

    `MMKV { file_size: 1024, key_count: 4, content_len: 107 }`
     */
    pub fn dump() -> String {
        kv_store!().to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::MMKV;

    #[test]
    fn test_mmkv() {
        let _ = fs::remove_file("mini_mmkv");
        let _ = fs::remove_file("mmkv_meta");
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
        let _ = fs::remove_file("mmkv_meta");
    }
}