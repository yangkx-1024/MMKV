mod ffi_buffer;

use crate::{Error, LogLevel, Logger, MMKV};
use ffi_buffer::{Leakable, Releasable};
use mmkv_proc_macro_lib::Leakable;
use std::ffi::{CStr, c_void};
use std::fmt::Debug;
use std::os::raw::c_char;

pub(super) const LOG_TAG: &str = "MMKV:FFI";

/// Error codes carried in `InternalError.code`.
///
/// `0..=7` mirror the library's `Error` variants one to one; the negative codes are
/// argument errors caught at the C boundary before MMKV runs. Every code except
/// `MMKV_ERR_KEY_NOT_FOUND` comes with a human readable `reason`.
pub const MMKV_ERR_KEY_NOT_FOUND: i32 = 0;
pub const MMKV_ERR_DECODE_FAILED: i32 = 1;
pub const MMKV_ERR_TYPE_MISS_MATCH: i32 = 2;
pub const MMKV_ERR_DATA_INVALID: i32 = 3;
pub const MMKV_ERR_INSTANCE_CLOSED: i32 = 4;
pub const MMKV_ERR_ENCODE_FAILED: i32 = 5;
/// The data file could not be written, for example because the disk is full.
pub const MMKV_ERR_IO: i32 = 6;
/// An internal lock was poisoned by a panic on another thread.
pub const MMKV_ERR_LOCK: i32 = 7;
/// `key` is null or not valid UTF-8.
pub const MMKV_ERR_INVALID_KEY: i32 = -1;
/// The instance pointer is null.
pub const MMKV_ERR_NULL_INSTANCE: i32 = -2;
/// `value` is null with a non-zero `len`, or not valid UTF-8 for `put_str`.
pub const MMKV_ERR_INVALID_VALUE: i32 = -3;

pub type CByteArray = *const u8;
pub type CI32Array = *const i32;
pub type CI64Array = *const i64;
pub type CF32Array = *const f32;
pub type CF64Array = *const f64;

/// The value type a `RawBuffer` carries; also tags a `RawTypedArray`.
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

/// `len` bytes at `bytes`, not NUL-terminated. Owned by the `RawBuffer` or
/// `InternalError` it hangs off and freed with it; never free it directly.
#[repr(C)]
#[derive(Debug, Leakable)]
pub struct ByteSlice {
    pub bytes: *const u8,
    pub len: usize,
    pub capacity: usize,
}

/// `len` elements of `type_token` at `array`. Owned by its `RawBuffer` and freed with it.
#[repr(C)]
#[derive(Debug, Leakable)]
pub struct RawTypedArray {
    pub array: *const c_void,
    pub type_token: Types,
    pub len: usize,
    pub capacity: usize,
}

/// A log sink implemented by the host, installed with `set_logger`.
///
/// MMKV writes its logs on a logger thread of its own, so `callback` and `destroy` run on
/// that thread, never on the thread that called into MMKV; both must be safe to call from
/// there. `content` is only valid for the duration of `callback`, copy it to keep it.
/// `destroy` runs exactly once, when a later `set_logger` replaces this logger.
#[repr(C)]
#[derive(Debug)]
pub struct NativeLogger {
    obj: *mut c_void,
    callback: extern "C" fn(obj: *mut c_void, level: i32, content: *const ByteSlice),
    destroy: extern "C" fn(obj: *mut c_void),
}

// SAFETY: MMKV never dereferences `obj`, it only hands it back to the two callbacks, and
// the contract on `NativeLogger` requires the host to make both callable from the logger
// thread. The logger is moved to that thread once and only ever used there.
unsafe impl Send for NativeLogger {}

// SAFETY: see `Send`. `Logger` requires `Sync`, and the only shared access is the logger
// thread calling `callback` through `&self`.
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

/// The result of a `put_*`, `get_*` or `delete` call, released with `free_buffer`.
///
/// `err == NULL` means success, and `raw_data` then points at the value for `get_*`: a
/// `ByteSlice` for `Str`, a `RawTypedArray` for the array types, the scalar itself
/// otherwise. `raw_data` is NULL for `put_*` and `delete`. Everything reachable from the
/// buffer is owned by it.
#[repr(C)]
#[derive(Debug, Leakable)]
pub struct RawBuffer {
    pub raw_data: *const c_void,
    pub type_token: Types,
    pub err: *const InternalError,
}

/// A failed call: `code` is one of the `MMKV_ERR_*` constants and `reason` is NULL or a
/// UTF-8 message, owned by the `RawBuffer`.
#[repr(C)]
#[derive(Debug, Leakable)]
pub struct InternalError {
    pub code: i32,
    pub reason: *const ByteSlice,
}

/// The `value` argument of a `put_*` entry point as the type `MMKV::put` takes, or the
/// argument error to report. Strings and arrays are borrowed from the caller's memory.
macro_rules! to_mmkv_value {
    ($value:expr, RawCStr) => {
        // SAFETY: the caller passes a NUL-terminated string that outlives the call, or null.
        unsafe { c_str_arg($value, "value") }
            .map_err(|reason| arg_error(MMKV_ERR_INVALID_VALUE, reason))
    };
    ($value:expr, bool) => {
        Ok::<bool, InternalError>($value)
    };
    ($value:expr, i32) => {
        Ok::<i32, InternalError>($value)
    };
    ($value:expr, i64) => {
        Ok::<i64, InternalError>($value)
    };
    ($value:expr, f32) => {
        Ok::<f32, InternalError>($value)
    };
    ($value:expr, f64) => {
        Ok::<f64, InternalError>($value)
    };
    ($value:expr, $len:expr, $array:tt) => {
        // SAFETY: the caller passes `len` readable elements that outlive the call, or null.
        unsafe { slice_arg($value, $len) }
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

/// Log and build the argument error `code` with `reason`.
fn arg_error(code: i32, reason: String) -> InternalError {
    error!(LOG_TAG, "{}", reason);
    InternalError::new(code, Some(reason))
}

/// The `MMKV` behind a handle from `new_instance`, or an error for a null handle.
///
/// # Safety
/// `ptr` must be null or a handle from `new_instance` that has not been closed.
unsafe fn instance<'a>(ptr: *const c_void) -> Result<&'a MMKV, InternalError> {
    // SAFETY: a non-null `ptr` is a live `MMKV` by the caller's guarantee.
    unsafe { (ptr as *const MMKV).as_ref() }.ok_or_else(|| {
        arg_error(
            MMKV_ERR_NULL_INSTANCE,
            "instance pointer is null".to_string(),
        )
    })
}

/// A C string argument named `what` as `&str`, or the reason it is unusable.
///
/// # Safety
/// `ptr` must be null or point at a NUL-terminated string that outlives the `&str`.
unsafe fn c_str_arg<'a>(ptr: RawCStr, what: &str) -> Result<&'a str, String> {
    if ptr.is_null() {
        return Err(format!("{what} is null"));
    }
    // SAFETY: non-null, and NUL-terminated by the caller's guarantee.
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map_err(|e| format!("{what} is not valid UTF-8: {e}"))
}

/// The `key` argument of an entry point.
///
/// # Safety
/// As for [`c_str_arg`].
unsafe fn key_arg<'a>(key: RawCStr) -> Result<&'a str, InternalError> {
    // SAFETY: forwarded from the caller.
    unsafe { c_str_arg(key, "key") }.map_err(|reason| arg_error(MMKV_ERR_INVALID_KEY, reason))
}

/// A `(pointer, len)` array argument as a slice. Null with `len == 0` is the empty array.
///
/// # Safety
/// `ptr` must be null or point at `len` initialised `T`s that outlive the slice.
unsafe fn slice_arg<'a, T>(ptr: *const T, len: usize) -> Result<&'a [T], InternalError> {
    if ptr.is_null() {
        return if len == 0 {
            Ok(&[])
        } else {
            Err(arg_error(
                MMKV_ERR_INVALID_VALUE,
                format!("value is null but len is {len}"),
            ))
        };
    }
    // SAFETY: non-null, and `len` valid elements by the caller's guarantee.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// Log a failed operation on `key` and convert it to its error code.
fn map_error(key: &str, e: Error, log: &str) -> InternalError {
    error!(LOG_TAG, "failed to {} for key {}, reason {:?}", log, key, e);
    e.into()
}

/// Hand the outcome of a `put`/`delete` back to C as a leaked `RawBuffer`.
fn reply_put(type_token: Types, outcome: Result<(), InternalError>) -> *const RawBuffer {
    let mut result = RawBuffer::new(type_token);
    if let Err(e) = outcome {
        result.set_error(e);
    }
    result.leak()
}

/// Hand the outcome of a `get` back to C as a leaked `RawBuffer` owning the value.
fn reply_get<T: Releasable + 'static>(
    type_token: Types,
    outcome: Result<T, InternalError>,
) -> *const RawBuffer {
    let mut result = RawBuffer::new(type_token);
    match outcome {
        Ok(value) => result.set_data(value),
        Err(e) => result.set_error(e),
    }
    result.leak()
}

/// Resolve the handle and key every keyed entry point takes, then run `op` on them.
/// Argument errors short-circuit into the returned outcome.
///
/// # Safety
/// `ptr` must be null or an open handle from `new_instance`; `key` must be null or a
/// NUL-terminated string that outlives the call.
unsafe fn entry<T>(
    ptr: *const c_void,
    key: RawCStr,
    op: impl FnOnce(&MMKV, &str) -> Result<T, InternalError>,
) -> Result<T, InternalError> {
    // SAFETY: forwarded from the caller.
    let mmkv = unsafe { instance(ptr) }?;
    // SAFETY: forwarded from the caller.
    let key_str = unsafe { key_arg(key) }?;
    op(mmkv, key_str)
}

macro_rules! impl_put {
    ($($name:ident, $value_type:tt, $type_token:expr, $log:literal;)+) => {
        $(
        /// Store `value` under `key`. The returned buffer carries no data, only a
        /// possible error; release it with `free_buffer`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            ptr: *const c_void,
            key: RawCStr,
            value: $value_type,
        ) -> *const RawBuffer {
            let op = |mmkv: &MMKV, key_str: &str| {
                let value = to_mmkv_value!(value, $value_type)?;
                mmkv.put(key_str, value)
                    .map_err(|e| map_error(key_str, e, $log))?;
                verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                Ok::<(), InternalError>(())
            };
            // SAFETY: `ptr` is an open handle from `new_instance` and `key` a
            // NUL-terminated string, both valid for this call or null.
            reply_put($type_token, unsafe { entry(ptr, key, op) })
        }
        )+
    };
}

macro_rules! impl_put_typed_array {
    ($($name:ident, $value_type:tt, $type_token:expr, $log:literal;)+) => {
        $(
        /// Store the `len` elements at `value` under `key`. The returned buffer carries
        /// no data, only a possible error; release it with `free_buffer`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(
            ptr: *const c_void,
            key: RawCStr,
            value: $value_type,
            len: usize,
        ) -> *const RawBuffer {
            let op = |mmkv: &MMKV, key_str: &str| {
                let value = to_mmkv_value!(value, len, $value_type)?;
                mmkv.put(key_str, value)
                    .map_err(|e| map_error(key_str, e, $log))?;
                verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                Ok::<(), InternalError>(())
            };
            // SAFETY: `ptr` is an open handle from `new_instance`, `key` a
            // NUL-terminated string and `value` `len` readable elements, all valid for
            // this call or null.
            reply_put($type_token, unsafe { entry(ptr, key, op) })
        }
        )+
    };
}

macro_rules! impl_get {
    ($($name:ident, $value_type:tt, $type_token:expr, $log:literal;)+) => {
        $(
        /// Read the value stored under `key`. On success the returned buffer owns the
        /// value in `raw_data`, on failure `err` is set; release it with `free_buffer`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(ptr: *const c_void, key: RawCStr) -> *const RawBuffer {
            let op = |mmkv: &MMKV, key_str: &str| {
                let value = mmkv_get!(mmkv, key_str, $value_type)
                    .map_err(|e| map_error(key_str, e, $log))?;
                verbose!(LOG_TAG, "{} for key '{}' success", $log, key_str);
                Ok::<_, InternalError>(value)
            };
            // SAFETY: `ptr` is an open handle from `new_instance` and `key` a
            // NUL-terminated string, both valid for this call or null.
            reply_get($type_token, unsafe { entry(ptr, key, op) })
        }
        )+
    };
}

/// Open the store in `dir`, a writable directory, and return an opaque handle for it.
///
/// Returns null when `dir` is null, not UTF-8 or not a writable directory; the reason
/// goes to the logger. Handles on the same directory share one store and may be used
/// from any thread. Release the handle with `close_instance` exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn new_instance(dir: *const c_char) -> *const c_void {
    // SAFETY: the caller passes a NUL-terminated string that outlives the call, or null.
    let dir_str = match unsafe { c_str_arg(dir, "dir") } {
        Ok(dir_str) => dir_str,
        Err(reason) => {
            error!(LOG_TAG, "{}", reason);
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

/// Install `logger` as the process-wide log sink, taking ownership of it. See
/// `NativeLogger` for the thread its callbacks run on.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_logger(logger: NativeLogger) {
    MMKV::set_logger(Box::new(logger));
}

/// Set the process-wide log level: 0 off, 1 error, 2 warn, 3 info, 4 debug, 5 verbose.
/// Any other value is logged and ignored.
#[unsafe(no_mangle)]
pub extern "C" fn set_log_level(log_level: i32) {
    match LogLevel::try_from(log_level) {
        Ok(level) => MMKV::set_log_level(level),
        Err(()) => error!(LOG_TAG, "ignoring unknown log level {}", log_level),
    }
}

/// Release a `RawBuffer` returned by `put_*`, `get_*` or `delete`, together with the
/// value or error it owns. Null is a no-op; releasing a buffer twice is undefined.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_buffer(ptr: *const c_void) {
    if ptr.is_null() {
        return;
    }
    (ptr as *mut RawBuffer).release();
}

/// Close a handle from `new_instance`. Null is a no-op. The handle must not be used or
/// closed again afterwards; the store itself stays open while other handles exist.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn close_instance(ptr: *const c_void) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: a non-null `ptr` is a handle from `new_instance` the caller closes once.
    drop(unsafe { Box::from_raw(ptr as *mut MMKV) });
}

/// Delete every record in the store behind `ptr` and keep it usable. A failure, or a
/// null `ptr`, is logged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn clear_data(ptr: *const c_void) {
    // SAFETY: `ptr` is an open handle from `new_instance`, or null.
    let mmkv = match unsafe { instance(ptr) } {
        Ok(mmkv) => mmkv,
        Err(_) => return,
    };
    if let Err(e) = mmkv.clear_data() {
        error!(LOG_TAG, "failed to clear MMKV data: {:?}", e);
    }
}

/// Delete `key`; deleting a missing key succeeds. Release the returned buffer with
/// `free_buffer`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn delete(ptr: *const c_void, key: RawCStr) -> *const RawBuffer {
    let op = |mmkv: &MMKV, key_str: &str| {
        mmkv.delete(key_str)
            .map_err(|e| map_error(key_str, e, "delete"))?;
        verbose!(LOG_TAG, "delete key {} success", key_str);
        Ok::<(), InternalError>(())
    };
    // SAFETY: `ptr` is an open handle from `new_instance` and `key` a NUL-terminated
    // string, both valid for this call or null.
    reply_put(Types::Str, unsafe { entry(ptr, key, op) })
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
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, MutexGuard};

    use tempfile::{TempDir, tempdir};

    use super::*;
    use crate::core::config::Config;
    use crate::core::test_support;
    use crate::log::logger;

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
                MMKV_ERR_KEY_NOT_FOUND
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
                MMKV_ERR_TYPE_MISS_MATCH
            );
            assert_eq!(
                expect_error(get_f64(mmkv.ptr, key.as_ptr())),
                MMKV_ERR_TYPE_MISS_MATCH
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
            assert_eq!(
                expect_error(get_i32(mmkv.ptr, key.as_ptr())),
                MMKV_ERR_KEY_NOT_FOUND
            );
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

            assert_eq!(
                expect_error(get_i32(mmkv.ptr, key.as_ptr())),
                MMKV_ERR_KEY_NOT_FOUND
            );
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
    fn set_log_level_reaches_the_logger_globals_and_ignores_unknown_levels() {
        let _guard = lock_logger();
        let level_before = logger::get_log_level();

        set_log_level(LogLevel::Warn as i32);
        assert_eq!(logger::get_log_level(), LogLevel::Warn as i32);
        set_log_level(LogLevel::Verbose as i32);
        assert_eq!(logger::get_log_level(), LogLevel::Verbose as i32);
        // An unknown level is logged and ignored, not an abort.
        set_log_level(99);
        set_log_level(-1);
        assert_eq!(logger::get_log_level(), LogLevel::Verbose as i32);

        set_log_level(level_before);
    }

    /// The code and reason of a failed call, releasing the buffer.
    fn expect_error_with_reason(raw: *const RawBuffer) -> (i32, Option<String>) {
        with_buffer(raw, |buffer| {
            assert!(!buffer.err.is_null(), "expected an error, got {buffer:?}");
            // SAFETY: `err` is non-null and points at a leaked `InternalError` whose
            // `reason`, when non-null, is a live `ByteSlice`.
            unsafe {
                let err = &*buffer.err;
                let reason = err.reason.as_ref().map(|slice| {
                    let bytes = std::slice::from_raw_parts(slice.bytes, slice.len);
                    String::from_utf8(bytes.to_vec()).unwrap()
                });
                (err.code, reason)
            }
        })
    }

    /// The value length whose byte-array record fills `page` exactly, measured with the
    /// real encoder like `tests/durability.rs` does.
    fn value_len_filling_a_page(path: &Path, page: u64, key: &str) -> usize {
        let target = page as i64 - test_support::HEADER_LEN as i64;
        let mut len = target - 32;
        for _ in 0..4 {
            assert!(len > 0, "page {page} is too small for this test");
            let value = vec![1u8; len as usize];
            let record = test_support::record_len(path, key, value.as_slice()) as i64;
            if record == target {
                return len as usize;
            }
            len += target - record;
        }
        panic!("could not size a value whose record fills exactly {page} bytes");
    }

    /// A write that fails inside MMKV (here the trim it needs cannot create its tmp file)
    /// comes back as `MMKV_ERR_IO` with a reason, instead of aborting the host process
    /// from an unmapped error variant.
    #[test]
    fn a_failed_write_reports_an_io_error_instead_of_aborting() {
        let mmkv = Instance::new();
        let data_file = mmkv.data_file();
        let key = c_string("k");
        let page = std::fs::metadata(&data_file).unwrap().len();
        let fill = value_len_filling_a_page(&data_file, page, "k");
        let first = vec![1u8; fill];
        let second = vec![2u8; fill];

        // SAFETY: the instance, the key and both arrays are valid for the whole test.
        unsafe {
            expect_ok(put_byte_array(mmkv.ptr, key.as_ptr(), first.as_ptr(), fill));
            // The page is full and the key is a duplicate, so the next put has to trim;
            // occupy the tmp path that trim would create.
            let config = Config::new(&data_file, page).unwrap();
            let blockers = test_support::block_next_trims(&config, 1);

            let (code, reason) = expect_error_with_reason(put_byte_array(
                mmkv.ptr,
                key.as_ptr(),
                second.as_ptr(),
                fill,
            ));
            assert_eq!(code, MMKV_ERR_IO);
            assert!(reason.is_some(), "an IO error carries its reason");
            // The old value is intact, and the store works again once the fault is gone.
            assert_eq!(
                expect_array::<u8>(get_byte_array(mmkv.ptr, key.as_ptr())),
                first
            );
            for blocker in blockers {
                std::fs::remove_file(blocker).unwrap();
            }
            expect_ok(put_byte_array(
                mmkv.ptr,
                key.as_ptr(),
                second.as_ptr(),
                fill,
            ));
            assert_eq!(
                expect_array::<u8>(get_byte_array(mmkv.ptr, key.as_ptr())),
                second
            );
        }
    }

    /// Every entry point rejects a null handle with `MMKV_ERR_NULL_INSTANCE`, or ignores
    /// it for the void ones, instead of dereferencing it.
    #[test]
    fn a_null_handle_reports_an_error_instead_of_aborting() {
        let key = c_string("k");
        let value = c_string("v");
        let bytes = [1u8, 2, 3];
        let null: *const c_void = std::ptr::null();

        // SAFETY: a null handle is accepted by contract; every other argument is valid
        // for the duration of the calls.
        unsafe {
            assert_eq!(
                expect_error(put_i32(null, key.as_ptr(), 1)),
                MMKV_ERR_NULL_INSTANCE
            );
            assert_eq!(
                expect_error(put_str(null, key.as_ptr(), value.as_ptr())),
                MMKV_ERR_NULL_INSTANCE
            );
            assert_eq!(
                expect_error(put_byte_array(
                    null,
                    key.as_ptr(),
                    bytes.as_ptr(),
                    bytes.len()
                )),
                MMKV_ERR_NULL_INSTANCE
            );
            assert_eq!(
                expect_error(get_i32(null, key.as_ptr())),
                MMKV_ERR_NULL_INSTANCE
            );
            assert_eq!(
                expect_error(get_str(null, key.as_ptr())),
                MMKV_ERR_NULL_INSTANCE
            );
            assert_eq!(
                expect_error(get_byte_array(null, key.as_ptr())),
                MMKV_ERR_NULL_INSTANCE
            );
            assert_eq!(
                expect_error(delete(null, key.as_ptr())),
                MMKV_ERR_NULL_INSTANCE
            );
            clear_data(null);
            close_instance(null);
            free_buffer(null);
        }
    }

    /// A null or non-UTF-8 key or value is an argument error, never a panic. The value
    /// checks cover the paths the key checks do not: `put_str` and the typed arrays.
    #[test]
    fn null_or_non_utf8_arguments_report_an_error_instead_of_aborting() {
        let mmkv = Instance::new();
        let key = c_string("k");
        let null_str: RawCStr = std::ptr::null();
        // Invalid UTF-8, NUL-terminated by `CString`.
        let bad = CString::new([0xffu8, 0xfe].as_slice()).unwrap();

        // SAFETY: the instance is open and every non-null string is NUL-terminated and
        // outlives the calls.
        unsafe {
            assert_eq!(
                expect_error(put_i32(mmkv.ptr, null_str, 1)),
                MMKV_ERR_INVALID_KEY
            );
            assert_eq!(
                expect_error(put_i32(mmkv.ptr, bad.as_ptr(), 1)),
                MMKV_ERR_INVALID_KEY
            );
            assert_eq!(
                expect_error(get_i32(mmkv.ptr, bad.as_ptr())),
                MMKV_ERR_INVALID_KEY
            );
            assert_eq!(
                expect_error(delete(mmkv.ptr, null_str)),
                MMKV_ERR_INVALID_KEY
            );

            assert_eq!(
                expect_error(put_str(mmkv.ptr, key.as_ptr(), null_str)),
                MMKV_ERR_INVALID_VALUE
            );
            assert_eq!(
                expect_error(put_str(mmkv.ptr, key.as_ptr(), bad.as_ptr())),
                MMKV_ERR_INVALID_VALUE
            );
            assert_eq!(
                expect_error(put_byte_array(mmkv.ptr, key.as_ptr(), std::ptr::null(), 3)),
                MMKV_ERR_INVALID_VALUE
            );
            // Nothing above reached the store.
            assert_eq!(
                expect_error(get_i32(mmkv.ptr, key.as_ptr())),
                MMKV_ERR_KEY_NOT_FOUND
            );

            // A null array with `len == 0` is simply the empty array.
            expect_ok(put_byte_array(mmkv.ptr, key.as_ptr(), std::ptr::null(), 0));
            assert_eq!(
                expect_array::<u8>(get_byte_array(mmkv.ptr, key.as_ptr())),
                Vec::<u8>::new()
            );
        }
    }
}
