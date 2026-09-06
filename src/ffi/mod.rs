mod ffi_buffer;

use crate::{Error, LogLevel, Logger, MMKV};
use ffi_buffer::{Leakable, Releasable};
use mmkv_proc_macro_lib::Leakable;
use std::ffi::{CStr, c_void};
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
    pub capacity: usize,
}

#[repr(C)]
#[derive(Debug, Leakable)]
pub struct RawTypedArray {
    pub array: *const c_void,
    pub type_token: Types,
    pub len: usize,
    pub capacity: usize,
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
        ptr.release();
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

macro_rules! to_mmkv_value {
    ($value:expr, RawCStr) => {
        unsafe {
            // SAFETY: we assume ffi caller passed valid c_char
            CStr::from_ptr($value)
        }
        .to_str()
        .unwrap()
    };
    ($value:expr, bool) => {
        $value
    };
    ($value:expr, i32) => {
        $value
    };
    ($value:expr, i64) => {
        $value
    };
    ($value:expr, f32) => {
        $value
    };
    ($value:expr, f64) => {
        $value
    };
    ($value:expr, $len:expr, CByteArray) => {
        unsafe { std::slice::from_raw_parts($value, $len) }
    };
    ($value:expr, $len:expr, CI32Array) => {
        unsafe { std::slice::from_raw_parts($value, $len) }
    };
    ($value:expr, $len:expr, CI64Array) => {
        unsafe { std::slice::from_raw_parts($value, $len) }
    };
    ($value:expr, $len:expr, CF32Array) => {
        unsafe { std::slice::from_raw_parts($value, $len) }
    };
    ($value:expr, $len:expr, CF64Array) => {
        unsafe { std::slice::from_raw_parts($value, $len) }
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
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            ptr: *const c_void,
            key: RawCStr,
            value: $value_type,
        ) -> *const RawBuffer {
            let mmkv = unsafe {
                // SAFETY: we assume ffi caller passed valid mmkv pointer
                (ptr as *const MMKV).as_ref()
            }.unwrap();
            let key_str_result = unsafe {
                // SAFETY: we assume ffi caller passed valid c_char
                CStr::from_ptr(key)
            }.to_str();
            let mut result = RawBuffer::new($type_token);
            match key_str_result {
                Ok(key_str) => {
                    match mmkv.put(key_str, to_mmkv_value!(value, $value_type)) {
                        Err(e) => result.set_error(map_error(key_str, e, $log)),
                        Ok(()) => {
                            verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                        }
                    }
                },
                Err(e) => {
                    let log_str = format!("Invalid key: {:?}", e);
                    error!(LOG_TAG, "{}", &log_str);
                    result.set_error(InternalError::new(-1, Some(log_str)));
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
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            ptr: *const c_void,
            key: RawCStr,
            value: $value_type,
            len: usize,
        ) -> *const RawBuffer {
            let mmkv = unsafe {
                // SAFETY: we assume ffi caller passed valid mmkv pointer
                (ptr as *const MMKV).as_ref()
            }.unwrap();
            let key_str_result = unsafe {
                // SAFETY: we assume ffi caller passed valid c_char
                CStr::from_ptr(key)
            }.to_str();
            let mut result = RawBuffer::new($type_token);
            match key_str_result {
                Ok(key_str) => {
                    match mmkv.put(key_str, to_mmkv_value!(value, len, $value_type)) {
                        Err(e) => result.set_error(map_error(key_str, e, $log)),
                        Ok(()) => {
                            verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                        }
                    }
                },
                Err(e) => {
                    let log_str = format!("Invalid key: {:?}", e);
                    error!(LOG_TAG, "{}", &log_str);
                    result.set_error(InternalError::new(-1, Some(log_str)));
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
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(ptr: *const c_void, key: RawCStr) -> *const RawBuffer {
            let mmkv = unsafe {
                // SAFETY: we assume ffi caller passed valid mmkv pointer
                (ptr as *const MMKV).as_ref()
            }.unwrap();
            let key_str_result = unsafe {
                // SAFETY: we assume ffi caller passed valid c_char
                CStr::from_ptr(key)
            }.to_str();
            let mut result = RawBuffer::new($type_token);
            match key_str_result {
                Ok(key_str) => {
                    match mmkv_get!(mmkv, key_str, $value_type) {
                        Err(e) => result.set_error(map_error(key_str, e, $log)),
                        Ok(value) => {
                            verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                            result.set_data(value);
                        }
                    }
                }
                Err(e) => {
                    let log_str = format!("Invalid key: {:?}", e);
                    error!(LOG_TAG, "{}", &log_str);
                    result.set_error(InternalError::new(-1, Some(log_str)));
                }
            }
            return result.leak();
        }
        )+
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn new_instance(dir: *const c_char) -> *const c_void {
    let dir_str_result = unsafe {
        // SAFETY: we assume ffi caller passed valid c_char
        CStr::from_ptr(dir)
    }
    .to_str();
    let dir_str = match dir_str_result {
        Ok(dir_str) => dir_str,
        Err(e) => {
            error!(LOG_TAG, "Invalid dir name: {:?}", e);
            return std::ptr::null();
        }
    };
    match MMKV::new(dir_str) {
        Ok(mmkv) => Box::into_raw(Box::new(mmkv)) as *const c_void,
        Err(e) => {
            error!(
                LOG_TAG,
                "failed to create MMKV instance for '{}': {:?}", dir_str, e
            );
            std::ptr::null()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_logger(logger: NativeLogger) {
    MMKV::set_logger(Box::new(logger));
}

#[unsafe(no_mangle)]
pub extern "C" fn set_log_level(log_level: i32) {
    MMKV::set_log_level(log_level.try_into().unwrap())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_buffer(ptr: *const c_void) {
    (ptr as *mut RawBuffer).release();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn close_instance(ptr: *const c_void) {
    unsafe {
        // SAFETY: we assume ffi caller passed valid mmkv pointer
        // Drop ptr
        let _ = Box::from_raw(ptr as *mut MMKV);
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn clear_data(ptr: *const c_void) {
    let mmkv = unsafe {
        // SAFETY: we assume ffi caller passed valid mmkv pointer
        (ptr as *const MMKV).as_ref()
    }
    .unwrap();
    if let Err(e) = mmkv.clear_data() {
        error!(LOG_TAG, "failed to clear MMKV data: {:?}", e);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn delete(ptr: *const c_void, key: RawCStr) -> *const RawBuffer {
    let mmkv = unsafe {
        // SAFETY: we assume ffi caller passed valid mmkv pointer
        (ptr as *const MMKV).as_ref()
    }
    .unwrap();
    let mut result = RawBuffer::new(Types::Str);
    let key_str_result = unsafe {
        // SAFETY: we assume ffi caller passed valid c_char
        CStr::from_ptr(key)
    }
    .to_str();
    match key_str_result {
        Ok(key_str) => match mmkv.delete(key_str) {
            Err(e) => result.set_error(map_error(key_str, e, "delete")),
            Ok(()) => verbose!(LOG_TAG, "delete key {} success", key_str),
        },
        Err(e) => {
            let log_str = format!("Invalid key: {:?}", e);
            error!(LOG_TAG, "{}", &log_str);
            result.set_error(InternalError::new(-1, Some(log_str)));
        }
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

/// Unit tests for the C API. The module only exists in non-android builds without the
/// encryption feature, so these tests run in the default flavour only.
///
/// Every `RawBuffer` a call hands back is released through `free_buffer`, so running the
/// suite under a normal `cargo test` also checks that nothing is freed twice.
#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, MutexGuard};

    use tempfile::{TempDir, tempdir};

    use super::*;
    use crate::log::logger;

    /// Error codes, mirrored from `impl TryFrom<Error> for InternalError`.
    const KEY_NOT_FOUND: i32 = 0;
    const TYPE_MISS_MATCH: i32 = 2;

    /// `set_logger` and `set_log_level` are process-wide, so the two tests that touch
    /// them must not interleave with each other.
    static LOGGER_LOCK: Mutex<()> = Mutex::new(());

    fn lock_logger() -> MutexGuard<'static, ()> {
        LOGGER_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn c_string(value: &str) -> CString {
        CString::new(value).unwrap()
    }

    /// An MMKV instance owned through the C API, closed again on drop.
    struct Instance {
        ptr: *const c_void,
        dir: TempDir,
    }

    impl Instance {
        fn new() -> Self {
            let dir = tempdir().unwrap();
            let c_dir = c_string(dir.path().to_str().unwrap());
            // SAFETY: `c_dir` is a valid NUL-terminated string that outlives the call.
            let ptr = unsafe { new_instance(c_dir.as_ptr()) };
            assert!(!ptr.is_null(), "new_instance on a temp dir must succeed");
            Instance { ptr, dir }
        }

        fn data_file(&self) -> std::path::PathBuf {
            self.dir.path().join("mini_mmkv")
        }
    }

    impl Drop for Instance {
        fn drop(&mut self) {
            // SAFETY: `ptr` came from `new_instance` and is closed exactly once.
            unsafe { close_instance(self.ptr) };
        }
    }

    /// Read something out of a returned `RawBuffer`, then release it.
    fn with_buffer<R>(raw: *const RawBuffer, read: impl FnOnce(&RawBuffer) -> R) -> R {
        assert!(!raw.is_null(), "the C API never returns a null RawBuffer");
        // SAFETY: `raw` was leaked by the C API and stays valid until `free_buffer` below.
        let out = read(unsafe { &*raw });
        // SAFETY: `raw` is a live leaked RawBuffer; it is released exactly once here.
        unsafe { free_buffer(raw as *const c_void) };
        out
    }

    /// Assert the call reported no error, then release the buffer.
    fn expect_ok(raw: *const RawBuffer) {
        with_buffer(raw, |buffer| {
            assert!(buffer.err.is_null(), "unexpected error: {buffer:?}");
        });
    }

    /// The error code of a failed call, releasing the buffer.
    fn expect_error(raw: *const RawBuffer) -> i32 {
        with_buffer(raw, |buffer| {
            assert!(!buffer.err.is_null(), "expected an error, got {buffer:?}");
            // SAFETY: `err` is non-null and points at a leaked `InternalError`.
            unsafe { (*buffer.err).code }
        })
    }

    /// The scalar payload of a successful call, releasing the buffer.
    fn expect_scalar<T: Copy>(raw: *const RawBuffer) -> T {
        with_buffer(raw, |buffer| {
            assert!(buffer.err.is_null(), "unexpected error: {buffer:?}");
            assert!(!buffer.raw_data.is_null());
            // SAFETY: a successful get leaks a `Box<T>` of the type this getter declares.
            unsafe { *(buffer.raw_data as *const T) }
        })
    }

    /// The string payload of a successful `get_str`, releasing the buffer.
    fn expect_string(raw: *const RawBuffer) -> String {
        with_buffer(raw, |buffer| {
            assert!(buffer.err.is_null(), "unexpected error: {buffer:?}");
            // SAFETY: a successful `get_str` leaks a `Box<ByteSlice>` whose bytes/len
            // describe a live `Vec<u8>` owned by that box.
            let bytes = unsafe {
                let slice = &*(buffer.raw_data as *const ByteSlice);
                std::slice::from_raw_parts(slice.bytes, slice.len)
            };
            String::from_utf8(bytes.to_vec()).unwrap()
        })
    }

    /// The typed-array payload of a successful call, releasing the buffer.
    fn expect_array<T: Copy>(raw: *const RawBuffer) -> Vec<T> {
        with_buffer(raw, |buffer| {
            assert!(buffer.err.is_null(), "unexpected error: {buffer:?}");
            // SAFETY: a successful array get leaks a `Box<RawTypedArray>` describing a
            // live `Vec<T>` of the element type this getter declares.
            unsafe {
                let array = &*(buffer.raw_data as *const RawTypedArray);
                std::slice::from_raw_parts(array.array as *const T, array.len).to_vec()
            }
        })
    }

    #[test]
    fn new_instance_rejects_a_path_that_is_not_a_dir() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("not_a_dir");
        std::fs::write(&file, b"x").unwrap();
        let c_file = c_string(file.to_str().unwrap());

        // SAFETY: `c_file` is a valid NUL-terminated string that outlives the call.
        let ptr = unsafe { new_instance(c_file.as_ptr()) };

        assert!(ptr.is_null(), "a file path must not yield an instance");
    }

    #[test]
    fn i32_roundtrips_and_a_missing_key_reports_key_not_found() {
        let mmkv = Instance::new();
        let key = c_string("i32_key");
        let missing = c_string("missing");

        // SAFETY: the instance and both keys are valid for the duration of the calls.
        unsafe {
            expect_ok(put_i32(mmkv.ptr, key.as_ptr(), -7));
            assert_eq!(expect_scalar::<i32>(get_i32(mmkv.ptr, key.as_ptr())), -7);
            assert_eq!(
                expect_error(get_i32(mmkv.ptr, missing.as_ptr())),
                KEY_NOT_FOUND
            );
        }
    }

    #[test]
    fn reading_a_key_as_the_wrong_type_reports_type_miss_match() {
        let mmkv = Instance::new();
        let key = c_string("typed_key");

        // SAFETY: the instance and the key are valid for the duration of the calls.
        unsafe {
            expect_ok(put_i32(mmkv.ptr, key.as_ptr(), 1));
            assert_eq!(
                expect_error(get_str(mmkv.ptr, key.as_ptr())),
                TYPE_MISS_MATCH
            );
            assert_eq!(
                expect_error(get_f64(mmkv.ptr, key.as_ptr())),
                TYPE_MISS_MATCH
            );
        }
    }

    #[test]
    fn every_scalar_type_roundtrips() {
        let mmkv = Instance::new();
        let (str_key, bool_key) = (c_string("str"), c_string("bool"));
        let (i64_key, f32_key, f64_key) = (c_string("i64"), c_string("f32"), c_string("f64"));
        let value = c_string("hello ffi");

        // SAFETY: the instance, keys and value string are valid for these calls.
        unsafe {
            expect_ok(put_str(mmkv.ptr, str_key.as_ptr(), value.as_ptr()));
            expect_ok(put_bool(mmkv.ptr, bool_key.as_ptr(), true));
            expect_ok(put_i64(mmkv.ptr, i64_key.as_ptr(), i64::MIN));
            expect_ok(put_f32(mmkv.ptr, f32_key.as_ptr(), 2.5f32));
            expect_ok(put_f64(mmkv.ptr, f64_key.as_ptr(), -0.125f64));

            assert_eq!(
                expect_string(get_str(mmkv.ptr, str_key.as_ptr())),
                "hello ffi"
            );
            assert!(expect_scalar::<bool>(get_bool(mmkv.ptr, bool_key.as_ptr())));
            assert_eq!(
                expect_scalar::<i64>(get_i64(mmkv.ptr, i64_key.as_ptr())),
                i64::MIN
            );
            assert_eq!(
                expect_scalar::<f32>(get_f32(mmkv.ptr, f32_key.as_ptr())),
                2.5
            );
            assert_eq!(
                expect_scalar::<f64>(get_f64(mmkv.ptr, f64_key.as_ptr())),
                -0.125
            );
        }
    }

    #[test]
    fn every_typed_array_roundtrips() {
        let mmkv = Instance::new();
        let bytes = vec![1u8, 2, 255];
        let i32s = vec![i32::MIN, 0, i32::MAX];
        let i64s = vec![i64::MIN, 0, i64::MAX];
        let f32s = vec![1.5f32, -2.5, 3.5];
        let f64s = vec![1.25f64, -2.25, 3.25];
        let (byte_key, i32_key) = (c_string("bytes"), c_string("i32s"));
        let (i64_key, f32_key, f64_key) = (c_string("i64s"), c_string("f32s"), c_string("f64s"));

        // SAFETY: every pointer/length pair describes a live slice for the whole call.
        unsafe {
            expect_ok(put_byte_array(
                mmkv.ptr,
                byte_key.as_ptr(),
                bytes.as_ptr(),
                bytes.len(),
            ));
            expect_ok(put_i32_array(
                mmkv.ptr,
                i32_key.as_ptr(),
                i32s.as_ptr(),
                i32s.len(),
            ));
            expect_ok(put_i64_array(
                mmkv.ptr,
                i64_key.as_ptr(),
                i64s.as_ptr(),
                i64s.len(),
            ));
            expect_ok(put_f32_array(
                mmkv.ptr,
                f32_key.as_ptr(),
                f32s.as_ptr(),
                f32s.len(),
            ));
            expect_ok(put_f64_array(
                mmkv.ptr,
                f64_key.as_ptr(),
                f64s.as_ptr(),
                f64s.len(),
            ));

            assert_eq!(
                expect_array::<u8>(get_byte_array(mmkv.ptr, byte_key.as_ptr())),
                bytes
            );
            assert_eq!(
                expect_array::<i32>(get_i32_array(mmkv.ptr, i32_key.as_ptr())),
                i32s
            );
            assert_eq!(
                expect_array::<i64>(get_i64_array(mmkv.ptr, i64_key.as_ptr())),
                i64s
            );
            assert_eq!(
                expect_array::<f32>(get_f32_array(mmkv.ptr, f32_key.as_ptr())),
                f32s
            );
            assert_eq!(
                expect_array::<f64>(get_f64_array(mmkv.ptr, f64_key.as_ptr())),
                f64s
            );
        }
    }

    #[test]
    fn delete_then_get_reports_key_not_found() {
        let mmkv = Instance::new();
        let key = c_string("doomed");

        // SAFETY: the instance and the key are valid for the duration of the calls.
        unsafe {
            expect_ok(put_i32(mmkv.ptr, key.as_ptr(), 3));
            expect_ok(delete(mmkv.ptr, key.as_ptr()));
            assert_eq!(expect_error(get_i32(mmkv.ptr, key.as_ptr())), KEY_NOT_FOUND);
            // Deleting a key that is already gone is not an error.
            expect_ok(delete(mmkv.ptr, key.as_ptr()));
        }
    }

    #[test]
    fn clear_data_empties_the_store_and_leaves_it_usable() {
        let mmkv = Instance::new();
        let key = c_string("cleared");

        // SAFETY: the instance and the key are valid for the duration of the calls.
        unsafe {
            expect_ok(put_i32(mmkv.ptr, key.as_ptr(), 9));
            clear_data(mmkv.ptr);

            assert_eq!(expect_error(get_i32(mmkv.ptr, key.as_ptr())), KEY_NOT_FOUND);
            // `MMKV::clear_data` reopens the store, so the data file is back but empty.
            let header = std::fs::read(mmkv.data_file()).unwrap();
            assert_eq!(&header[..8], &0u64.to_be_bytes());

            expect_ok(put_i32(mmkv.ptr, key.as_ptr(), 10));
            assert_eq!(expect_scalar::<i32>(get_i32(mmkv.ptr, key.as_ptr())), 10);
        }
    }

    static LOG_CALLS: AtomicUsize = AtomicUsize::new(0);
    static LOG_DESTROYS: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn count_log(_obj: *mut c_void, _level: i32, content: *const ByteSlice) {
        assert!(!content.is_null(), "the logger is always handed a payload");
        LOG_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    extern "C" fn count_destroy(_obj: *mut c_void) {
        LOG_DESTROYS.fetch_add(1, Ordering::Relaxed);
    }

    fn counting_logger() -> NativeLogger {
        NativeLogger {
            obj: std::ptr::null_mut(),
            callback: count_log,
            destroy: count_destroy,
        }
    }

    /// Other tests log into the same process while this one runs, so only "more than
    /// before" can be asserted, never an exact count.
    #[test]
    fn a_native_logger_receives_logs_and_is_destroyed_when_replaced() {
        let _guard = lock_logger();
        let level_before = logger::get_log_level();
        set_log_level(LogLevel::Verbose as i32);

        // SAFETY: the logger owns no foreign object (`obj` is null), so both callbacks
        // are safe to call with it.
        unsafe { set_logger(counting_logger()) };
        let calls_before = LOG_CALLS.load(Ordering::Relaxed);
        let destroys_before = LOG_DESTROYS.load(Ordering::Relaxed);

        let mmkv = Instance::new();
        let key = c_string("logged");
        // SAFETY: the instance and the key are valid for the duration of the call.
        unsafe { expect_ok(put_i32(mmkv.ptr, key.as_ptr(), 1)) };
        logger::sync().unwrap();
        assert!(
            LOG_CALLS.load(Ordering::Relaxed) > calls_before,
            "the installed logger must receive MMKV's own logs"
        );

        // Installing a second logger destroys the first.
        // SAFETY: same as above.
        unsafe { set_logger(counting_logger()) };
        logger::sync().unwrap();
        assert!(
            LOG_DESTROYS.load(Ordering::Relaxed) > destroys_before,
            "replacing a logger must destroy the one it replaces"
        );

        logger::set_logger(None);
        logger::sync().unwrap();
        set_log_level(level_before);
    }

    #[test]
    fn set_log_level_reaches_the_logger_globals() {
        let _guard = lock_logger();
        let level_before = logger::get_log_level();

        set_log_level(LogLevel::Warn as i32);
        assert_eq!(logger::get_log_level(), LogLevel::Warn as i32);
        set_log_level(LogLevel::Verbose as i32);
        assert_eq!(logger::get_log_level(), LogLevel::Verbose as i32);

        set_log_level(level_before);
    }
}
