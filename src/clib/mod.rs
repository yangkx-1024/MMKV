#[cfg(any(target_os = "ios", target_os = "macos"))]
#[cfg(not(feature = "encryption"))]
#[allow(non_snake_case)]
pub mod clib {
    use std::ffi::CStr;
    use std::os::raw::{c_char, c_int};
    use crate::MMKV;

    const LOG_TAG: &str = "MMKV:iOS";

    #[no_mangle]
    pub extern fn initialize(dir: *const c_char) {
        let dir_str = unsafe { CStr::from_ptr(dir) }.to_str().unwrap();
        MMKV::initialize(dir_str)
    }

    #[no_mangle]
    pub extern fn put_i32(key: *const c_char, value: i32) {
        let key_str = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
        match MMKV::put_i32(key_str, value) {
            Err(_) => {}
            Ok(()) => {
                verbose!(LOG_TAG, "put i32 for key '{}' success", key_str);
            }
        }
    }

    #[no_mangle]
    pub extern fn get_i32(key: *const c_char) -> c_int {
        let key_str = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
        return match MMKV::get_i32(key_str) {
            Err(_) => { 0 }
            Ok(value) => {
                verbose!(LOG_TAG, "get i32 for key '{}' success, value '{}'", key_str, value);
                value
            }
        }
    }
}