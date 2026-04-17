use std::any::TypeId;
use std::fmt::Debug;

use crate::Error;
use crate::ffi::*;

pub(super) trait Releasable: Debug {
    fn release(&mut self);
}

pub(super) trait Leakable<T: Releasable>: Debug {
    fn leak(self) -> *mut T;
}

impl<T> Leakable<T> for T
where
    T: 'static + Releasable,
{
    fn leak(self) -> *mut T {
        let log = format!("{:?}", self);
        let ptr = Box::into_raw(Box::new(self));
        if TypeId::of::<T>() != TypeId::of::<ByteSlice>() {
            verbose!(LOG_TAG, "leak {log}, ptr: {:?}", ptr);
        }
        ptr
    }
}

impl<T> Releasable for *mut T
where
    T: 'static + Debug,
{
    fn release(&mut self) {
        let ptr = *self;
        let log = format!("{:?}", ptr);
        let boxed = unsafe { Box::from_raw(ptr) };
        if TypeId::of::<T>() != TypeId::of::<ByteSlice>() {
            verbose!(LOG_TAG, "release {:?}, ptr: {}", boxed, log);
        }
        drop(boxed);
    }
}

macro_rules! impl_release_for_primary {
    ($($ident:ident),+) => {
        $(
            impl Releasable for $ident {
                fn release(&mut self) {
                    // Do nothing, we need nothing to be release inside primary type
                }
            }
        )+
    };
}

impl_release_for_primary!(bool, i32, i64, f32, f64);

impl ByteSlice {
    pub(super) fn new(string: String) -> Self {
        let (bytes, len, capacity) = string.into_bytes().into_raw_parts();
        ByteSlice {
            bytes,
            len,
            capacity,
        }
    }
}

impl Releasable for ByteSlice {
    fn release(&mut self) {
        unsafe {
            let _ = Vec::from_raw_parts(self.bytes as *mut u8, self.len, self.capacity);
        };
    }
}

impl RawTypedArray {
    pub(super) fn new<T: Debug>(array: Vec<T>, type_token: Types) -> Self {
        let log = format!("{:?}", array);
        let (ptr, len, capacity) = array.into_raw_parts();
        verbose!(LOG_TAG, "leak {log}, ptr: {:?}", ptr);
        RawTypedArray {
            array: ptr as *const _,
            type_token,
            len,
            capacity,
        }
    }
}

macro_rules! release_array {
    ($target:expr, $type:ty) => {{
        unsafe {
            let array =
                Vec::from_raw_parts($target.array as *mut $type, $target.len, $target.capacity);
            verbose!(LOG_TAG, "release {:?}, ptr: {:?}", array, array.as_ptr());
            drop(array);
        }
    }};
}

impl Releasable for RawTypedArray {
    fn release(&mut self) {
        match self.type_token {
            Types::ByteArray => release_array!(self, u8),
            Types::I32Array => release_array!(self, i32),
            Types::I64Array => release_array!(self, i64),
            Types::F32Array => release_array!(self, f32),
            Types::F64Array => release_array!(self, f64),
            _ => {
                panic!("can't match type of array")
            }
        };
    }
}

impl RawBuffer {
    pub(super) fn new(type_token: Types) -> Self {
        RawBuffer {
            raw_data: std::ptr::null(),
            type_token,
            err: std::ptr::null(),
        }
    }

    pub(super) fn set_data<T>(&mut self, data: T)
    where
        T: Releasable + 'static,
    {
        self.raw_data = data.leak() as *const _;
    }

    unsafe fn drop_data(&mut self) {
        if self.raw_data.is_null() {
            return;
        }
        match self.type_token {
            Types::I32 => (self.raw_data as *mut i32).release(),
            Types::Str => (self.raw_data as *mut ByteSlice).release(),
            Types::Bool => (self.raw_data as *mut bool).release(),
            Types::I64 => (self.raw_data as *mut i64).release(),
            Types::F32 => (self.raw_data as *mut f32).release(),
            Types::F64 => (self.raw_data as *mut f64).release(),
            Types::ByteArray
            | Types::I32Array
            | Types::I64Array
            | Types::F32Array
            | Types::F64Array => (self.raw_data as *mut RawTypedArray).release(),
        };
    }

    pub(super) fn set_error(&mut self, e: InternalError) {
        self.err = e.leak();
    }

    unsafe fn drop_error(&mut self) {
        if !self.err.is_null() {
            self.err.cast_mut().release();
        }
    }
}

impl Releasable for RawBuffer {
    fn release(&mut self) {
        unsafe {
            self.drop_data();
            self.drop_error();
        }
    }
}

impl InternalError {
    pub(super) fn new(code: i32, reason: Option<String>) -> Self {
        match reason {
            None => InternalError {
                code,
                reason: std::ptr::null(),
            },
            Some(str) => {
                let byte_slice = ByteSlice::new(str);
                let log = format!("{:?}", byte_slice);
                let reason = byte_slice.leak();
                verbose!(LOG_TAG, "leak {log}, ptr: {:?}", reason);
                InternalError { code, reason }
            }
        }
    }
}

impl TryFrom<Error> for InternalError {
    type Error = ();

    fn try_from(e: Error) -> Result<Self, Self::Error> {
        match e {
            Error::KeyNotFound => Ok(InternalError::new(0, None)),
            Error::DecodeFailed(descr) => Ok(InternalError::new(1, Some(descr))),
            Error::TypeMissMatch => Ok(InternalError::new(2, None)),
            Error::DataInvalid => Ok(InternalError::new(3, None)),
            Error::InstanceClosed => Ok(InternalError::new(4, None)),
            Error::EncodeFailed(descr) => Ok(InternalError::new(5, Some(descr))),
            _ => unreachable!("should not happen"),
        }
    }
}

impl Releasable for InternalError {
    fn release(&mut self) {
        if !self.reason.is_null() {
            unsafe {
                verbose!(
                    LOG_TAG,
                    "release ByteSlice {{ bytes: {:?}, len: {}, capacity: {} }}, ptr: {:?}",
                    (*self.reason).bytes,
                    (*self.reason).len,
                    (*self.reason).capacity,
                    self.reason
                );
            }
            self.reason.cast_mut().release();
        }
    }
}

#[cfg(test)]
mod test {
    use std::ffi::c_void;

    use crate::ffi::ffi_buffer::{Leakable, Releasable};
    use crate::ffi::{ByteSlice, InternalError, RawBuffer, RawTypedArray, Types, free_buffer};
    use crate::log::logger;

    #[test]
    fn test_byte_slice_empty() {
        let slice = ByteSlice::new(String::new());
        assert_eq!(slice.len, 0);
        assert_eq!(slice.capacity, 0);
        let mut ptr = slice.leak();
        ptr.release();
        logger::sync().unwrap()
    }

    #[test]
    fn test_byte_slice_non_empty() {
        let slice = ByteSlice::new("Test slice".to_string());
        assert_eq!(slice.len, 10);
        assert!(slice.capacity >= slice.len);
        let mut ptr = slice.leak();
        ptr.release();
        logger::sync().unwrap()
    }

    #[test]
    fn test_internal_error() {
        let str = "Test slice".to_string();
        let mut ptr = InternalError::new(0, Some(str)).leak();
        ptr.release();
        logger::sync().unwrap();
    }

    #[test]
    fn test_raw_typed_array_empty() {
        let array = RawTypedArray::new(Vec::<i32>::new(), Types::I32Array);
        assert_eq!(array.len, 0);
        assert_eq!(array.capacity, 0);
        let mut ptr = array.leak();
        ptr.release();
        logger::sync().unwrap();
    }

    #[test]
    fn test_raw_typed_array_non_empty() {
        let array = RawTypedArray::new(vec![1i32, 2, 3], Types::I32Array);
        assert_eq!(array.len, 3);
        assert!(array.capacity >= array.len);
        let mut ptr = array.leak();
        ptr.release();
        logger::sync().unwrap();
    }

    fn free_data_buffer<T: super::Releasable + 'static>(type_token: Types, data: T) {
        let mut buffer = RawBuffer::new(type_token);
        buffer.set_data(data);
        unsafe { free_buffer(buffer.leak() as *const c_void) };
    }

    #[test]
    fn test_raw_buffer_release_via_free_buffer() {
        let mut buffer = RawBuffer::new(Types::Bool);
        buffer.set_error(InternalError::new(0, None));
        unsafe { free_buffer(buffer.leak() as *const c_void) };

        free_data_buffer(Types::Str, ByteSlice::new("test str".to_string()));
        free_data_buffer(Types::Str, ByteSlice::new(String::new()));
        free_data_buffer(Types::I32, 10i32);
        free_data_buffer(
            Types::I32Array,
            RawTypedArray::new(vec![1i32, 2, 3], Types::I32Array),
        );
        free_data_buffer(
            Types::I32Array,
            RawTypedArray::new(Vec::<i32>::new(), Types::I32Array),
        );
        logger::sync().unwrap();
    }
}
