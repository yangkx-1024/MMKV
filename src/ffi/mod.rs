mod ffi_buffer;

use crate::{Error, LogLevel, Logger, MMKV};
use ffi_buffer::{Leakable, Releasable};
use mmkv_proc_macro_lib::Leakable;
use std::ffi::{c_void, CStr};
use std::fmt::Debug;
use std::os::raw::c_char;

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
#[derive(Debug, Leakable)]
pub struct ByteSlice {
    pub bytes: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug, Leakable)]
pub struct RawTypedArray {
    pub array: *const c_void,
    pub type_token: Types,
    pub len: usize,
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
        let mut ptr = ByteSlice::new(log_str).leak();
        (self.callback)(self.obj, log_level as i32, ptr);
        unsafe {
            ptr.release();
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
#[derive(Debug, Leakable)]
pub struct RawBuffer {
    pub raw_data: *const c_void,
    pub type_token: Types,
    pub err: *const InternalError,
}

#[repr(C)]
#[derive(Debug, Leakable)]
pub struct InternalError {
    pub code: i32,
    pub reason: *const ByteSlice,
}

macro_rules! mmkv_put {
    ($mmkv:ident, $key:expr, $value:expr, RawCStr) => {{
        let value_str = CStr::from_ptr($value).to_str().unwrap();
        $mmkv.put($key, value_str)
    }};
    ($mmkv:ident, $key:expr, $value:expr, bool) => {
        $mmkv.put($key, $value)
    };
    ($mmkv:ident, $key:expr, $value:expr, i32) => {
        $mmkv.put($key, $value)
    };
    ($mmkv:ident, $key:expr, $value:expr, i64) => {
        $mmkv.put($key, $value)
    };
    ($mmkv:ident, $key:expr, $value:expr, f32) => {
        $mmkv.put($key, $value)
    };
    ($mmkv:ident, $key:expr, $value:expr, f64) => {
        $mmkv.put($key, $value)
    };
    ($mmkv:ident, $key:expr, $value:expr, $len:expr, CByteArray) => {
        $mmkv.put($key, std::slice::from_raw_parts($value, $len))
    };
    ($mmkv:ident, $key:expr, $value:expr, $len:expr, CI32Array) => {
        $mmkv.put($key, std::slice::from_raw_parts($value, $len))
    };
    ($mmkv:ident, $key:expr, $value:expr, $len:expr, CI64Array) => {
        $mmkv.put($key, std::slice::from_raw_parts($value, $len))
    };
    ($mmkv:ident, $key:expr, $value:expr, $len:expr, CF32Array) => {
        $mmkv.put($key, std::slice::from_raw_parts($value, $len))
    };
    ($mmkv:ident, $key:expr, $value:expr, $len:expr, CF64Array) => {
        $mmkv.put($key, std::slice::from_raw_parts($value, $len))
    };
}

macro_rules! mmkv_get {
    ($mmkv:ident, $key:expr, ByteSlice) => {
        $mmkv.get($key).map(|value| ByteSlice::new(value))
    };
    ($mmkv:ident, $key:expr, bool) => {
        $mmkv.get::<bool>($key)
    };
    ($mmkv:ident, $key:expr, i32) => {
        $mmkv.get::<i32>($key)
    };
    ($mmkv:ident, $key:expr, i64) => {
        $mmkv.get::<i64>($key)
    };
    ($mmkv:ident, $key:expr, f32) => {
        $mmkv.get::<f32>($key)
    };
    ($mmkv:ident, $key:expr, f64) => {
        $mmkv.get::<f64>($key)
    };
    ($mmkv:ident, $key:expr, CByteArray) => {
        $mmkv
            .get::<Vec<u8>>($key)
            .map(|value| RawTypedArray::new(value, Types::ByteArray))
    };
    ($mmkv:ident, $key:expr, CI32Array) => {
        $mmkv
            .get::<Vec<i32>>($key)
            .map(|value| RawTypedArray::new(value, Types::I32Array))
    };
    ($mmkv:ident, $key:expr, CI64Array) => {
        $mmkv
            .get::<Vec<i64>>($key)
            .map(|value| RawTypedArray::new(value, Types::I64Array))
    };
    ($mmkv:ident, $key:expr, CF32Array) => {
        $mmkv
            .get::<Vec<f32>>($key)
            .map(|value| RawTypedArray::new(value, Types::F32Array))
    };
    ($mmkv:ident, $key:expr, CF64Array) => {
        $mmkv
            .get::<Vec<f64>>($key)
            .map(|value| RawTypedArray::new(value, Types::F64Array))
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
    ($($name:ident, $value_type:tt, $type_token:expr, $log:literal;)+) => {
        $(
        #[no_mangle]
        pub unsafe extern "C" fn $name(
            ptr: *const c_void,
            key: RawCStr,
            value: $value_type,
        ) -> *const RawBuffer {
            let mmkv = (ptr as *const MMKV).as_ref().unwrap();
            let key_str = CStr::from_ptr(key).to_str().unwrap();
            let mut result = RawBuffer::new($type_token);
            match mmkv_put!(mmkv, key_str, value, $value_type) {
                Err(e) => result.set_error(map_error(key_str, e, $log)),
                Ok(()) => {
                    verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                }
            }
            result.leak()
        }
        )+
    };
}

macro_rules! impl_put_typed_array {
    ($($name:ident, $value_type:tt, $type_token:expr, $log:literal;)+) => {
        $(
        #[no_mangle]
        pub unsafe extern "C" fn $name(
            ptr: *const c_void,
            key: RawCStr,
            value: $value_type,
            len: usize,
        ) -> *const RawBuffer {
            let mmkv = (ptr as *const MMKV).as_ref().unwrap();
            let key_str = CStr::from_ptr(key).to_str().unwrap();
            let mut result = RawBuffer::new($type_token);
            match mmkv_put!(mmkv, key_str, value, len, $value_type) {
                Err(e) => result.set_error(map_error(key_str, e, $log)),
                Ok(()) => {
                    verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                }
            }
            result.leak()
        }
        )+
    };
}

macro_rules! impl_get {
    ($($name:ident, $value_type:tt, $type_token:expr, $log:literal;)+) => {
        $(
        #[no_mangle]
        pub unsafe extern "C" fn $name(ptr: *const c_void, key: RawCStr) -> *const RawBuffer {
            let mmkv = (ptr as *const MMKV).as_ref().unwrap();
            let key_str = CStr::from_ptr(key).to_str().unwrap();
            let mut result = RawBuffer::new($type_token);
            match mmkv_get!(mmkv, key_str, $value_type) {
                Err(e) => result.set_error(map_error(key_str, e, $log)),
                Ok(value) => {
                    verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                    result.set_data(value);
                }
            }
            return result.leak();
        }
        )+
    };
}

#[no_mangle]
pub extern "C" fn new_instance(dir: *const c_char) -> *const c_void {
    let dir_str = unsafe { CStr::from_ptr(dir) }.to_str().unwrap();
    let mmkv = MMKV::new(dir_str);
    Box::into_raw(Box::new(mmkv)) as *const c_void
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
    (ptr as *mut RawBuffer).release();
}

#[no_mangle]
pub unsafe extern "C" fn close_instance(ptr: *const c_void) {
    drop(Box::from_raw(ptr as *mut MMKV));
}

#[no_mangle]
pub unsafe extern "C" fn clear_data(ptr: *const c_void) {
    let mmkv = (ptr as *const MMKV).as_ref().unwrap();
    mmkv.clear_data();
}

#[no_mangle]
pub unsafe extern "C" fn delete(ptr: *const c_void, key: RawCStr) -> *const RawBuffer {
    let mmkv = (ptr as *const MMKV).as_ref().unwrap();
    let key_str = unsafe { CStr::from_ptr(key) }.to_str().unwrap();
    let mut result = RawBuffer::new(Types::Str);
    match mmkv.delete(key_str) {
        Err(e) => result.set_error(map_error(key_str, e, "delete")),
        Ok(()) => verbose!(LOG_TAG, "delete key {} success", key_str),
    }
    result.leak()
}

impl_put!(
    put_str, RawCStr, Types::Str, "put string";
    put_bool, bool, Types::Bool, "put bool";
    put_i32, i32, Types::I32, "put i32";
    put_i64, i64, Types::I64, "put i64";
    put_f32, f32, Types::F32, "put f32";
    put_f64, f64, Types::F64, "put f64";
);

impl_get!(
    get_str, ByteSlice, Types::Str, "get string";
    get_bool, bool, Types::Bool, "get bool";
    get_i32, i32, Types::I32, "get i32";
    get_i64, i64, Types::I64, "get i64";
    get_f32, f32, Types::F32, "get f32";
    get_f64, f64, Types::F64, "get f64";
    get_byte_array, CByteArray, Types::ByteArray, "get byte array";
    get_i32_array, CI32Array, Types::I32Array, "get i32 array";
    get_i64_array, CI64Array, Types::I64Array, "get i64 array";
    get_f32_array, CF32Array, Types::F32Array, "get f32 array";
    get_f64_array, CF64Array, Types::F64Array, "get f64 array";
);

impl_put_typed_array!(
    put_byte_array, CByteArray, Types::ByteArray, "put byte array";
    put_i32_array, CI32Array, Types::I32Array, "put i32 array";
    put_i64_array, CI64Array, Types::I64Array, "put i64 array";
    put_f32_array, CF32Array, Types::F32Array, "put f32 array";
    put_f64_array, CF64Array, Types::F64Array, "put f64 array";
);
