mod ffi_buffer;

use mmkv_proc_macro_lib::AutoRelease;
use std::ffi::{c_void, CStr};
use std::fmt::Debug;
use std::os::raw::c_char;

use crate::{Error, LogLevel, Logger, MMKV};

pub(super) const LOG_TAG: &str = "MMKV:FFI";

pub type CByteArray = *const u8;
pub type CI32Array = *const i32;
pub type CI64Array = *const i64;
pub type CF32Array = *const f32;
pub type CF64Array = *const f64;

#[repr(C)]
#[derive(Debug)]
#[allow(dead_code)]
pub enum Types {
    I32,
    Str,
    Bool,
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
#[derive(Debug, AutoRelease)]
pub struct ByteSlice {
    pub bytes: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug, AutoRelease)]
pub struct RawTypedArray {
    pub array: *const c_void,
    pub type_token: Types,
    pub len: usize,
}

#[no_mangle]
pub extern "C" fn __use_typed_array(typed_array: RawTypedArray) {
    error!(LOG_TAG, "{:?}", typed_array)
}

#[repr(C)]
#[derive(Debug)]
pub struct NativeLogger {
    obj: *mut c_void,
    callback: extern "C" fn(obj: *mut c_void, level: i32, content: *const ByteSlice),
    destroy: extern "C" fn(obj: *mut c_void),
}

unsafe impl Send for NativeLogger {}

unsafe impl Sync for NativeLogger {}

impl Drop for NativeLogger {
    fn drop(&mut self) {
        verbose!(LOG_TAG, "release {:?}", self);
        (self.destroy)(self.obj);
    }
}

impl NativeLogger {
    fn call_target(&self, log_level: LogLevel, log_str: String) {
        let ptr = Box::into_raw(Box::new(ByteSlice::new(log_str)));
        (self.callback)(self.obj, log_level as i32, ptr);
        unsafe {
            let _ = Box::from_raw(ptr);
        }
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
#[derive(Debug, AutoRelease)]
pub struct RawBuffer {
    pub raw_data: *const c_void,
    pub type_token: Types,
    pub err: *const InternalError,
}

#[repr(C)]
#[derive(Debug, AutoRelease)]
pub struct InternalError {
    pub code: i32,
    pub reason: *const ByteSlice,
}

macro_rules! mmkv_put {
    ($key:expr, $value:expr, RawCStr) => {{
        let value_str = unsafe { CStr::from_ptr($value) }.to_str().unwrap();
        MMKV::put_str($key, value_str)
    }};
    ($key:expr, $value:expr, bool) => {
        MMKV::put_bool($key, $value)
    };
    ($key:expr, $value:expr, i32) => {
        MMKV::put_i32($key, $value)
    };
    ($key:expr, $value:expr, i64) => {
        MMKV::put_i64($key, $value)
    };
    ($key:expr, $value:expr, f32) => {
        MMKV::put_f32($key, $value)
    };
    ($key:expr, $value:expr, f64) => {
        MMKV::put_f64($key, $value)
    };
    ($key:expr, $value:expr, $len:expr, CByteArray) => {
        MMKV::put_byte_array($key, unsafe { std::slice::from_raw_parts($value, $len) })
    };
    ($key:expr, $value:expr, $len:expr, CI32Array) => {
        MMKV::put_i32_array($key, unsafe { std::slice::from_raw_parts($value, $len) })
    };
    ($key:expr, $value:expr, $len:expr, CI64Array) => {
        MMKV::put_i64_array($key, unsafe { std::slice::from_raw_parts($value, $len) })
    };
    ($key:expr, $value:expr, $len:expr, CF32Array) => {
        MMKV::put_f32_array($key, unsafe { std::slice::from_raw_parts($value, $len) })
    };
    ($key:expr, $value:expr, $len:expr, CF64Array) => {
        MMKV::put_f64_array($key, unsafe { std::slice::from_raw_parts($value, $len) })
    };
}

macro_rules! mmkv_get {
    ($key:expr, ByteSlice) => {
        MMKV::get_str($key).map(|value| ByteSlice::new(value))
    };
    ($key:expr, bool) => {
        MMKV::get_bool($key)
    };
    ($key:expr, i32) => {
        MMKV::get_i32($key)
    };
    ($key:expr, i64) => {
        MMKV::get_i64($key)
    };
    ($key:expr, f32) => {
        MMKV::get_f32($key)
    };
    ($key:expr, f64) => {
        MMKV::get_f64($key)
    };
    ($key:expr, CByteArray) => {
        MMKV::get_byte_array($key).map(|value| RawTypedArray::new(value, Types::ByteArray))
    };
    ($key:expr, CI32Array) => {
        MMKV::get_i32_array($key).map(|value| RawTypedArray::new(value, Types::I32Array))
    };
    ($key:expr, CI64Array) => {
        MMKV::get_i64_array($key).map(|value| RawTypedArray::new(value, Types::I64Array))
    };
    ($key:expr, CF32Array) => {
        MMKV::get_f32_array($key).map(|value| RawTypedArray::new(value, Types::F32Array))
    };
    ($key:expr, CF64Array) => {
        MMKV::get_f64_array($key).map(|value| RawTypedArray::new(value, Types::F64Array))
    };
}

fn map_error(key: &str, e: Error, log: &str) -> InternalError {
    error!(
        LOG_TAG,
        "{}",
        format!("failed to {} for key {}, reason {:?}", log, key, e)
    );
    e.try_into().unwrap()
}

macro_rules! impl_put {
    ($name:ident, $value_type:tt, $type_token:expr, $log:literal) => {
        #[no_mangle]
        pub extern "C" fn $name(key: RawCStr, value: $value_type) -> *const RawBuffer {
            let key_str = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
            let mut result = RawBuffer::new($type_token);
            match mmkv_put!(key_str, value, $value_type) {
                Err(e) => result.set_error(map_error(key_str, e, $log)),
                Ok(()) => {
                    verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                }
            }
            result.leak()
        }
    };
}

macro_rules! impl_put_typed_array {
    ($name:ident, $value_type:tt, $type_token:expr, $log:literal) => {
        #[no_mangle]
        pub extern "C" fn $name(key: RawCStr, value: $value_type, len: usize) -> *const RawBuffer {
            let key_str = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
            let mut result = Box::new(RawBuffer::new($type_token));
            match mmkv_put!(key_str, value, len, $value_type) {
                Err(e) => result.set_error(map_error(key_str, e, $log)),
                Ok(()) => {
                    verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                }
            }
            result.leak()
        }
    };
}

macro_rules! impl_get {
    ($name:ident, $value_type:tt, $type_token:expr, $log:literal) => {
        #[no_mangle]
        pub extern "C" fn $name(key: RawCStr) -> *const RawBuffer {
            let key_str = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
            let mut result = RawBuffer::new($type_token);
            match mmkv_get!(key_str, $value_type) {
                Err(e) => result.set_error(map_error(key_str, e, $log)),
                Ok(value) => {
                    verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                    result.set_data(value);
                }
            }
            return result.leak();
        }
    };
}

#[no_mangle]
pub extern "C" fn initialize(dir: *const c_char) {
    let dir_str = unsafe { CStr::from_ptr(dir) }.to_str().unwrap();
    MMKV::initialize(dir_str)
}

#[no_mangle]
pub extern "C" fn set_logger(logger: NativeLogger) {
    MMKV::set_logger(Box::new(logger));
}

#[no_mangle]
pub extern "C" fn set_log_level(log_level: i32) {
    MMKV::set_log_level(log_level.try_into().unwrap())
}

#[no_mangle]
pub unsafe extern "C" fn free_buffer(ptr: *const c_void) {
    let _ = RawBuffer::from_raw(ptr as *mut RawBuffer);
}

#[no_mangle]
pub extern "C" fn close_instance() {
    MMKV::close()
}

#[no_mangle]
pub extern "C" fn clear_data() {
    MMKV::clear_data()
}

impl_put!(put_str, RawCStr, Types::Str, "put string");

impl_get!(get_str, ByteSlice, Types::Str, "get string");

impl_put!(put_bool, bool, Types::Bool, "put bool");

impl_get!(get_bool, bool, Types::Bool, "get bool");

impl_put!(put_i32, i32, Types::I32, "put i32");

impl_get!(get_i32, i32, Types::I32, "get i32");

impl_put!(put_i64, i64, Types::I64, "put i64");

impl_get!(get_i64, i64, Types::I64, "get i64");

impl_put!(put_f32, f32, Types::F32, "put f32");

impl_get!(get_f32, f32, Types::F32, "get f32");

impl_put!(put_f64, f64, Types::F64, "put f64");

impl_get!(get_f64, f64, Types::F64, "get f64");

impl_put_typed_array!(
    put_byte_array,
    CByteArray,
    Types::ByteArray,
    "put byte array"
);

impl_get!(
    get_byte_array,
    CByteArray,
    Types::ByteArray,
    "get byte array"
);

impl_put_typed_array!(put_i32_array, CI32Array, Types::I32Array, "put i32 array");

impl_get!(get_i32_array, CI32Array, Types::I32Array, "get i32 array");

impl_put_typed_array!(put_i64_array, CI64Array, Types::I64Array, "put i64 array");

impl_get!(get_i64_array, CI64Array, Types::I64Array, "get i64 array");

impl_put_typed_array!(put_f32_array, CF32Array, Types::F32Array, "put f32 array");

impl_get!(get_f32_array, CF32Array, Types::F32Array, "get f32 array");

impl_put_typed_array!(put_f64_array, CF64Array, Types::F64Array, "put f64 array");

impl_get!(get_f64_array, CF64Array, Types::F64Array, "get f64 array");
