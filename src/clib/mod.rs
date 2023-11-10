#[cfg(any(target_os = "ios", target_os = "macos"))]
#[cfg(not(feature = "encryption"))]
#[allow(non_snake_case)]
pub mod clib {
    use std::any::Any;
    use std::ffi::{c_void, CStr};
    use std::fmt::Debug;
    use std::os::raw::c_char;

    use crate::{Error, Logger, LogLevel, MMKV};

    const LOG_TAG: &str = "MMKV:CLIB";

    #[repr(C)]
    #[derive(Debug)]
    #[allow(dead_code)]
    pub enum Types {
        I32,
        STR,
        BYTE,
        I64,
        F32,
        F64,
        ByteArray,
        I32Array,
        I64Array,
        F32Array,
        F64Array,
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct ByteSlice {
        pub bytes: *const u8,
        pub len: usize,
    }

    impl ByteSlice {
        fn new(string: String) -> Self {
            let boxed = string.into_boxed_str();
            let ptr = boxed.as_ptr();
            let len = boxed.len();
            std::mem::forget(boxed);
            ByteSlice {
                bytes: ptr,
                len,
            }
        }

        fn leak(self) -> *mut ByteSlice {
            Box::into_raw(Box::new(self))
        }

        fn from_raw(ptr: *mut ByteSlice) -> Self {
            *unsafe {
                Box::from_raw(ptr)
            }
        }
    }

    impl Drop for ByteSlice {
        fn drop(&mut self) {
            unsafe {
                let _ = String::from_raw_parts(
                    self.bytes as *mut u8, self.len, self.len,
                );
            };
        }
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct RawTypedArray {
        pub array: *const c_void,
        pub type_token: Types,
        pub len: usize,
    }

    impl RawTypedArray {
        fn new_with_i32_array(array: Vec<i32>) -> Self {
            let boxed = array.into_boxed_slice();
            let ptr = boxed.as_ptr();
            let len = boxed.len();
            std::mem::forget(boxed);
            RawTypedArray {
                array: ptr as *mut _,
                type_token: Types::I32Array,
                len,
            }
        }

        unsafe fn drop_array(&mut self) {
            let _: Box<dyn Any> = match self.type_token {
                Types::I32Array => {
                    Box::from_raw(
                        std::slice::from_raw_parts_mut(
                            self.array as *mut i32, self.len,
                        ).as_mut_ptr()
                    )
                }
                _ => { Box::new(()) }
            };
        }
    }

    impl Drop for RawTypedArray {
        fn drop(&mut self) {
            unsafe {
                self.drop_array()
            }
        }
    }

    #[no_mangle]
    pub extern fn __use_typed_array(typed_array: RawTypedArray) {
        error!(LOG_TAG, "{:?}", typed_array)
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct NativeLogger {
        obj: *mut c_void,
        callback: extern fn(obj: *mut c_void, level: i32, content: *const ByteSlice),
    }

    unsafe impl Send for NativeLogger {}

    unsafe impl Sync for NativeLogger {}

    impl NativeLogger {
        fn call_target(&self, log_level: LogLevel, log_str: String) {
            let ptr = ByteSlice::new(log_str).leak();
            (self.callback)(
                self.obj, log_level as i32, ptr,
            );
            ByteSlice::from_raw(ptr);
        }
    }

    impl Logger for NativeLogger {
        fn verbose(&self, log_str: String) {
            self.call_target(LogLevel::Verbose, log_str);
        }

        fn info(&self, log_str: String) {
            self.call_target(LogLevel::Info, log_str);
        }

        fn debug(&self, log_str: String) {
            self.call_target(LogLevel::Debug, log_str);
        }

        fn warn(&self, log_str: String) {
            self.call_target(LogLevel::Warn, log_str);
        }

        fn error(&self, log_str: String) {
            self.call_target(LogLevel::Error, log_str);
        }
    }

    pub type RawCStr = *const c_char;

    #[repr(C)]
    #[derive(Debug)]
    pub struct RawBuffer {
        pub rawData: *const c_void,
        pub typeToken: Types,
        pub err: *const InternalError,
    }

    impl RawBuffer {
        fn new(type_token: Types) -> Self {
            RawBuffer {
                rawData: std::ptr::null(),
                typeToken: type_token,
                err: std::ptr::null(),
            }
        }

        fn set_data(&mut self, data: Box<dyn Any>) {
            self.rawData = Box::into_raw(data) as *const _
        }

        unsafe fn drop_data(&mut self) {
            if self.rawData.is_null() {
                return;
            }
            let _: Box<dyn Any> = match self.typeToken {
                Types::STR => {
                    Box::from_raw(self.rawData as *mut ByteSlice)
                }
                Types::I32 => {
                    Box::from_raw(self.rawData as *mut i32)
                }
                Types::I32Array => {
                    Box::from_raw(self.rawData as *mut RawTypedArray)
                }
                _ => { Box::new(()) }
            };
        }

        fn set_error(&mut self, e: Box<InternalError>) {
            self.err = Box::into_raw(e);
        }

        unsafe fn drop_error(&mut self) {
            if !self.err.is_null() {
                let _ = Box::from_raw(self.err.cast_mut());
            }
        }

        fn leak(self) -> *mut RawBuffer {
            Box::into_raw(Box::new(self))
        }

        fn from_raw(ptr: *mut RawBuffer) -> Self {
            *unsafe {
                Box::from_raw(ptr)
            }
        }
    }

    impl Drop for RawBuffer {
        fn drop(&mut self) {
            unsafe {
                self.drop_data();
                self.drop_error();
            }
        }
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct InternalError {
        pub code: i32,
        pub reason: *const ByteSlice,
    }

    impl InternalError {
        fn new(code: i32, reason: Option<String>) -> Self {
            match reason {
                None => {
                    InternalError {
                        code,
                        reason: std::ptr::null(),
                    }
                }
                Some(str) => {
                    InternalError {
                        code,
                        reason: ByteSlice::new(str).leak(),
                    }
                }
            }
        }
    }

    impl Drop for InternalError {
        fn drop(&mut self) {
            if !self.reason.is_null() {
                let _ = ByteSlice::from_raw(self.reason.cast_mut());
            }
        }
    }

    impl TryFrom<Error> for InternalError {
        type Error = ();

        #[allow(unreachable_patterns)]
        fn try_from(e: Error) -> Result<Self, Self::Error> {
            match e {
                Error::KeyNotFound => Ok(InternalError::new(0, None)),
                Error::DecodeFailed(descr) => Ok(InternalError::new(1, Some(descr))),
                Error::TypeMissMatch => Ok(InternalError::new(2, None)),
                Error::DataInvalid => Ok(InternalError::new(3, None)),
                Error::InstanceClosed => Ok(InternalError::new(4, None)),
                Error::EncodeFailed(descr) => Ok(InternalError::new(5, Some(descr))),
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
        ($key:expr, $value:expr, $len:expr, Int32Array) => {{
            let array = unsafe {
                std::slice::from_raw_parts($value, $len)
            };
            MMKV::put_i32_array($key, array)
        }};
    }

    macro_rules! mmkv_get {
        ($key:expr, ByteSlice) => {
            MMKV::get_str($key).map(|value| ByteSlice::new(value))
        };
        ($key:expr, i32) => {
            MMKV::get_i32($key)
        };
        ($key:expr, Int32Array) => {
            MMKV::get_i32_array($key).map(|value| RawTypedArray::new_with_i32_array(value))
        }
    }

    fn map_error(key: &str, e: Error, log: &str) -> Box<InternalError> {
        error!(LOG_TAG, "{}", format!("failed to {} for key {}, reason {:?}", log, key, e));
        return Box::new(e.try_into().unwrap());
    }

    macro_rules! impl_put {
        ($name:ident, $value_type:tt, $type_token:expr, $log:literal) => {
            #[no_mangle]
            pub extern fn $name(key: RawCStr, value: $value_type) -> *const RawBuffer {
                let key_str = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
                let mut result = RawBuffer::new($type_token);
                match mmkv_put!(key_str, value, $value_type) {
                    Err(e) => {
                        result.set_error(map_error(key_str, e, $log))
                    }
                    Ok(()) => {
                        verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                    }
                }
                return result.leak();
            }
        }
    }

    macro_rules! impl_put_typed_array {
        ($name:ident, $value_type:tt, $type_token:expr, $log:literal) => {
            #[no_mangle]
            pub extern fn $name(key: RawCStr, value: $value_type, len: usize) -> *const RawBuffer {
                let key_str = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
                let mut result = Box::new(RawBuffer::new($type_token));
                match mmkv_put!(key_str, value, len, $value_type) {
                    Err(e) => {
                        result.set_error(map_error(key_str, e, $log))
                    }
                    Ok(()) => {
                        verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                    }
                }
                return result.leak();
            }
        }
    }

    macro_rules! impl_get {
        ($name:ident, $value_type:tt, $type_token:expr, $log:literal) => {
            #[no_mangle]
            pub extern fn $name(key: RawCStr) -> *const RawBuffer {
                let key_str = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
                let mut result = RawBuffer::new($type_token);
                match mmkv_get!(key_str, $value_type) {
                    Err(e) => {
                        result.set_error(map_error(key_str, e, $log))
                    }
                    Ok(value) => {
                        verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                        result.set_data(Box::new(value));
                    }
                }
                return result.leak();
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
    pub unsafe extern fn free_buffer(ptr: *const c_void) {
        let _ = RawBuffer::from_raw(ptr as *mut RawBuffer);
    }

    impl_put!(put_str, RawCStr, Types::STR, "put string");

    impl_get!(get_str, ByteSlice, Types::STR, "get string");

    impl_put!(put_i32, i32, Types::I32, "put i32");

    impl_get!(get_i32, i32, Types::I32, "get i32");

    type Int32Array = *const i32;
    impl_put_typed_array!(put_i32_array, Int32Array, Types::I32Array, "put i32 array");

    impl_get!(get_i32_array, Int32Array, Types::I32Array, "get i32 array");
}