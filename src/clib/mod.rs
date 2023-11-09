#[cfg(any(target_os = "ios", target_os = "macos"))]
#[cfg(not(feature = "encryption"))]
#[allow(non_snake_case)]
pub mod clib {
    use std::ffi::{c_void, CStr};
    use std::fmt::Debug;
    use std::os::raw::c_char;

    use crate::{Logger, LogLevel, MMKV};

    const LOG_TAG: &str = "MMKV:iOS";

    #[repr(C)]
    #[derive(Debug)]
    pub struct ByteSlice {
        pub bytes: *const u8,
        pub len: usize,
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct NativeLogger{
        obj: *mut c_void,
        callback: extern fn(obj: *mut c_void, level: i32, content: ByteSlice),
    }

    unsafe impl Send for NativeLogger {}

    unsafe impl Sync for NativeLogger {}

    impl Logger for NativeLogger {
        fn verbose(&self, log_str: &str) {
            (self.callback)(self.obj, LogLevel::Verbose as i32, ByteSlice {
                bytes: log_str.as_ptr(),
                len: log_str.len(),
            });
        }

        fn info(&self, log_str: &str) {
            (self.callback)(self.obj, LogLevel::Info as i32, ByteSlice {
                bytes: log_str.as_ptr(),
                len: log_str.len(),
            });
        }

        fn debug(&self, log_str: &str) {
            (self.callback)(self.obj, LogLevel::Debug as i32, ByteSlice {
                bytes: log_str.as_ptr(),
                len: log_str.len(),
            });
        }

        fn warn(&self, log_str: &str) {
            (self.callback)(self.obj, LogLevel::Warn as i32, ByteSlice {
                bytes: log_str.as_ptr(),
                len: log_str.len(),
            });
        }

        fn error(&self, log_str: &str) {
            (self.callback)(self.obj, LogLevel::Error as i32, ByteSlice {
                bytes: log_str.as_ptr(),
                len: log_str.len(),
            });
        }
    }

    pub type RawCStr = *const c_char;

    #[repr(C)]
    #[derive(Debug)]
    pub struct Result<T: Sized + Debug> {
        pub rawData: *const T,
        pub err: *const InternalError,
    }

    impl <T> Drop for Result<T> where T : Sized + Debug {
        fn drop(&mut self) {
            info!(LOG_TAG, "drop Result {:?}", self);
            if !self.rawData.is_null() {
                unsafe {
                    let _ = Box::from_raw(self.rawData.cast_mut());
                }
            }
            if !self.err.is_null() {
                unsafe {
                    let _ = Box::from_raw(self.err.cast_mut());
                }
            }
        }
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct VoidResult {
        pub err: *const InternalError,
    }

    impl Drop for VoidResult {
        fn drop(&mut self) {
            info!(LOG_TAG, "drop VoidResult {:?}", self);
            if !self.err.is_null() {
                unsafe {
                    let _ = Box::from_raw(self.err.cast_mut());
                }
            }
        }
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct InternalError {
        pub code: i32,
        pub reason: *const ByteSlice,
    }

    impl Drop for InternalError {
        fn drop(&mut self) {
            info!(LOG_TAG, "drop InternalError {:?}", self);
            if !self.reason.is_null() {
                unsafe {
                    let _ = Box::from_raw(self.reason.cast_mut());
                }
            }
        }
    }

    impl TryFrom<crate::Error> for InternalError {
        type Error = ();

        #[allow(unreachable_patterns)]
        fn try_from(e: crate::Error) -> std::result::Result<Self, Self::Error> {
            match e {
                crate::Error::KeyNotFound => Ok(InternalError {
                    code: 0,
                    reason: std::ptr::null(),
                }),
                crate::Error::DecodeFailed(descr) => {
                    let reason = Box::new(ByteSlice {
                        bytes: descr.as_ptr(),
                        len: descr.len()
                    });
                    Ok(InternalError {
                        code: 1,
                        reason: Box::into_raw(reason),
                    })
                }
                crate::Error::TypeMissMatch => Ok(InternalError {
                    code: 2,
                    reason: std::ptr::null(),
                }),
                crate::Error::DataInvalid => Ok(InternalError {
                    code: 3,
                    reason: std::ptr::null(),
                }),
                crate::Error::InstanceClosed => Ok(InternalError {
                    code: 4,
                    reason: std::ptr::null(),
                }),
                crate::Error::EncodeFailed(descr) => {
                    let reason = Box::new(ByteSlice {
                        bytes: descr.as_ptr(),
                        len: descr.len()
                    });
                    Ok(InternalError {
                        code: 5,
                        reason: Box::into_raw(reason),
                    })
                }
                _ => Err(())
            }
        }
    }

    macro_rules! mmkv_put {
        ($key:expr, $value:expr, RawCStr) => {{
            let value_str = unsafe { CStr::from_ptr($value) }.to_str().unwrap();
            MMKV::put_str($key, value_str)
        }};
        ($key:expr, $value:expr, i32) => {
            MMKV::put_i32($key, $value)
        };
    }

    macro_rules! mmkv_get {
        ($key:expr, ByteSlice) => {
            MMKV::get_str($key).map(|value| ByteSlice {
                bytes: value.as_ptr(),
                len: value.len(),
            })
        };
        ($key:expr, i32) => {
            MMKV::get_i32($key)
        }
    }

    macro_rules! impl_put {
        ($name:ident, $value_type:tt, $log_type:literal) => {
            #[no_mangle]
            pub extern fn $name(key: RawCStr, value: $value_type) -> *const VoidResult {
                let key_str = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
                let mut result = Box::new(VoidResult {
                    err: std::ptr::null()
                });
                match mmkv_put!(key_str, value, $value_type) {
                    Err(e) => {
                        let log_str = format!(
                            "failed to put {} for key {}, reason {:?}",
                            $log_type, key_str, e
                        );
                        error!(LOG_TAG, "{}", log_str);
                        result.err = Box::into_raw(Box::new(e.try_into().unwrap()))
                    }
                    Ok(()) => {
                        verbose!(LOG_TAG, "put {} for key '{}' success", $log_type, key_str);
                    }
                }
                return Box::into_raw(result);
            }
        }
    }

    macro_rules! impl_get {
        ($name:ident, $value_type:tt, $log_type:literal) => {
            #[no_mangle]
            pub extern fn $name(key: RawCStr) -> *const Result<$value_type> {
                let key_str = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
                let mut result = Box::new(Result {
                    rawData: std::ptr::null(),
                    err: std::ptr::null(),
                });
                match mmkv_get!(key_str, $value_type) {
                    Err(e) => {
                        let log_str = format!(
                            "failed to get {} for key {}, reason {:?}",
                            $log_type, key_str, e
                        );
                        error!(LOG_TAG, "{}", log_str);
                        result.err = Box::into_raw(Box::new(e.try_into().unwrap()))
                    }
                    Ok(value) => {
                        verbose!(LOG_TAG, "get {} for key '{}' success", $log_type, key_str);
                        result.rawData = Box::into_raw(Box::new(value))
                    }
                }
                return Box::into_raw(result)
            }
        }
    }

    macro_rules! impl_destroy_result {
        ($name:ident, $type:tt) => {
            #[no_mangle]
            pub unsafe extern fn $name(ptr: *const Result<$type>) {
                let _ = Box::from_raw(ptr.cast_mut());
            }
        }
    }

    #[no_mangle]
    pub extern fn initialize(dir: *const c_char, logger: NativeLogger) {
        let dir_str = unsafe { CStr::from_ptr(dir) }.to_str().unwrap();
        MMKV::set_logger(Box::new(logger));
        MMKV::initialize(dir_str)
    }

    #[no_mangle]
    pub unsafe extern fn destroy_void_result(ptr: *const VoidResult) {
        let _ = Box::from_raw(ptr.cast_mut());
    }

    impl_put!(put_str, RawCStr, "string");

    impl_get!(get_str, ByteSlice, "string");

    impl_destroy_result!(destroy_str_result, ByteSlice);

    impl_put!(put_i32, i32, "i32");

    impl_get!(get_i32, i32, "i32");

    impl_destroy_result!(destroy_i32_result, i32);
}