use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::core::buffer::Buffer;
use crate::core::mmkv_impl::MmkvImpl;

const DEFAULT_FILE_NAME: &str = "mini_mmkv";
const PAGE_SIZE: u64 = 4 * 1024; // 4KB is the default Linux page size

static mut MMKV_IMPL: OnceLock<MmkvImpl> = OnceLock::new();

macro_rules! mmkv {
    () => {
        {
            unsafe {
                MMKV_IMPL.get().expect("not initialize")
            }
        }
    }
}

macro_rules! mut_mmkv {
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

MMKV::initialize(".", #[cfg(feature = "encryption")] "88C51C536176AD8A8EE4A06F62EE897E");
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

    If enabled feature "encryption", additional param `key` is required,
    the key should be a hexadecimal string of length 16, for example:

    `88C51C536176AD8A8EE4A06F62EE897E`
     */
    pub fn initialize(dir: &str, #[cfg(feature = "encryption")] key: &str) {
        let file_path = MMKV::resolve_file_path(dir);
        unsafe {
            MMKV_IMPL.set(
                MmkvImpl::new(file_path.as_path(), PAGE_SIZE, #[cfg(feature = "encryption")] key)
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
        path.join(DEFAULT_FILE_NAME)
    }

    pub fn put_str(key: &str, value: &str) {
        mut_mmkv!().write(key, Buffer::from_str(key, value));
    }

    pub fn get_str(key: &str) -> Option<&str> {
        mmkv!().get(key).map(|buffer| {
            buffer.decode_str()
        }).flatten()
    }

    pub fn put_i32(key: &str, value: i32) {
        mut_mmkv!().write(key, Buffer::from_i32(key, value));
    }

    pub fn get_i32(key: &str) -> Option<i32> {
        mmkv!().get(key).map(|buffer| {
            buffer.decode_i32()
        }).flatten()
    }

    pub fn put_bool(key: &str, value: bool) {
        mut_mmkv!().write(key, Buffer::from_bool(key, value));
    }

    pub fn get_bool(key: &str) -> Option<bool> {
        mmkv!().get(key).map(|buffer| {
            buffer.decode_bool()
        }).flatten()
    }

    /**
    Dump the current state of MMKV, the result looks like this:

    `MMKV { file_size: 1024, key_count: 4, content_len: 107 }`
     */
    pub fn dump() -> String {
        mmkv!().to_string()
    }
}
