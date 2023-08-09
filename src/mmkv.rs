use log::info;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::core::buffer::Buffer;
use crate::core::mmkv_impl::MmkvImpl;

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
        MMKV::close();
        let file_path = MMKV::resolve_file_path(dir);
        let mmkv_impl = MmkvImpl::new(
            file_path.as_path(),
            PAGE_SIZE,
            #[cfg(feature = "encryption")]
            key,
        );
        let raw_ptr = Box::into_raw(Box::new(mmkv_impl));
        MMKV_IMPL.swap(raw_ptr, Ordering::Release);
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

    pub fn put_str(key: &str, value: &str) {
        mut_mmkv!().put(key, Buffer::from_str(key, value));
    }

    pub fn get_str(key: &str) -> Option<&str> {
        mmkv!().get(key).map(|buffer| buffer.decode_str()).flatten()
    }

    pub fn put_i32(key: &str, value: i32) {
        mut_mmkv!().put(key, Buffer::from_i32(key, value));
    }

    pub fn get_i32(key: &str) -> Option<i32> {
        mmkv!().get(key).map(|buffer| buffer.decode_i32()).flatten()
    }

    pub fn put_bool(key: &str, value: bool) {
        mut_mmkv!().put(key, Buffer::from_bool(key, value));
    }

    pub fn get_bool(key: &str) -> Option<bool> {
        mmkv!()
            .get(key)
            .map(|buffer| buffer.decode_bool())
            .flatten()
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
        let p = MMKV_IMPL.swap(std::ptr::null_mut(), Ordering::Release);
        if p != std::ptr::null_mut() {
            unsafe {
                drop(Box::from_raw(p));
                info!("old instance dropped");
            }
        }
    }

    /**
    Dump the current state of MMKV, the result looks like this:

    `MMKV { file_size: 1024, key_count: 4, content_len: 107 }`
     */
    pub fn dump() -> String {
        mmkv!().to_string()
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
        MMKV::close();
        assert_eq!(MMKV_IMPL.load(Ordering::Acquire), std::ptr::null_mut());
        MMKV::initialize(
            ".",
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        assert_eq!(MMKV::get_str("first"), Some("one"));
        MMKV::clear_data();
        assert_eq!(Path::new("./mini_mmkv").exists(), false);
        assert_eq!(Path::new("./mini_mmkv.meta").exists(), false);
        assert_eq!(MMKV_IMPL.load(Ordering::Acquire), std::ptr::null_mut());
    }
}
