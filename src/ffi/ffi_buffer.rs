use std::any::TypeId;
use std::fmt::Debug;
use std::ops::DerefMut;

use crate::ffi::*;
use crate::Error;

pub(super) trait Releasable: Debug {
    unsafe fn release(&mut self);
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
    unsafe fn release(&mut self) {
        let ptr = *self;
        let log = format!("{:?}", ptr);
        let boxed = Box::from_raw(ptr);
        if TypeId::of::<T>() != TypeId::of::<ByteSlice>() {
            verbose!(LOG_TAG, "release {:?}, ptr: {}", boxed, log);
        }
        drop(boxed);
    }
}

impl<T> Releasable for &mut [T]
where
    T: 'static + Debug,
{
    unsafe fn release(&mut self) {
        let ptr = self.deref_mut();
        let boxed = Box::from_raw(ptr);
        verbose!(LOG_TAG, "release {:?}, ptr: {:?}", boxed, ptr.as_ptr());
        drop(boxed);
    }
}

macro_rules! impl_release_for_primary {
    ($($ident:ident),+) => {
        $(
            impl Releasable for $ident {
                unsafe fn release(&mut self) {
                    // Do nothing, we need nothing to be release inside primary type
                }
            }
        )+
    };
}

impl_release_for_primary!(bool, i32, i64, f32, f64);

impl ByteSlice {
    pub(super) fn new(string: String) -> Self {
        let boxed = string.into_boxed_str();
        let ptr = boxed.as_ptr();
        let len = boxed.len();
        std::mem::forget(boxed);
        ByteSlice { bytes: ptr, len }
    }
}

impl Releasable for ByteSlice {
    unsafe fn release(&mut self) {
        unsafe {
            let _ = String::from_raw_parts(self.bytes as *mut u8, self.len, self.len);
        };
    }
}

impl RawTypedArray {
    pub(super) fn new<T: Debug>(array: Vec<T>, type_token: Types) -> Self {
        let boxed = array.into_boxed_slice();
        let ptr = boxed.as_ptr();
        let len = boxed.len();
        verbose!(LOG_TAG, "leak {:?}, ptr: {:?}", boxed, ptr);
        std::mem::forget(boxed);
        RawTypedArray {
            array: ptr as *mut _,
            type_token,
            len,
        }
    }
}

macro_rules! release_array {
    ($target:expr, $type:ty) => {{
        std::slice::from_raw_parts_mut($target.array as *mut $type, $target.len).release();
    }};
}

impl Releasable for RawTypedArray {
    unsafe fn release(&mut self) {
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
    unsafe fn release(&mut self) {
        self.drop_data();
        self.drop_error();
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
    unsafe fn release(&mut self) {
        if !self.reason.is_null() {
            verbose!(
                LOG_TAG,
                "release ByteSlice {{ bytes: {:?}, len: {} }}, ptr: {:?}",
                (*self.reason).bytes,
                (*self.reason).len,
                self.reason
            );
            self.reason.cast_mut().release();
        }
    }
}

#[cfg(test)]
mod test {
    use crate::ffi::ffi_buffer::{Leakable, Releasable};
    use crate::ffi::{ByteSlice, InternalError, RawBuffer, RawTypedArray, Types};
    use crate::log::logger;

    #[test]
    fn test_byte_slice() {
        let str = "Test slice".to_string();
        let mut ptr = ByteSlice::new(str).leak();
        unsafe {
            ptr.release();
        }
        logger::sync().unwrap()
    }

    #[test]
    fn test_internal_error() {
        let str = "Test slice".to_string();
        let mut ptr = InternalError::new(0, Some(str)).leak();
        unsafe {
            ptr.release();
        }
        logger::sync().unwrap();
    }

    #[test]
    fn test_raw_buffer() {
        let mut buffer = RawBuffer::new(Types::Bool);
        buffer.set_error(InternalError::new(0, None));
        let mut ptr = buffer.leak();
        unsafe {
            ptr.release();
        }

        let mut buffer = RawBuffer::new(Types::Str);
        buffer.set_data(ByteSlice::new("test str".to_string()));
        let mut ptr = buffer.leak();
        unsafe {
            ptr.release();
        }

        let mut buffer = RawBuffer::new(Types::I32);
        buffer.set_data(10i32);
        let mut ptr = buffer.leak();
        unsafe {
            ptr.release();
        }

        let mut buffer = RawBuffer::new(Types::I32Array);
        buffer.set_data(RawTypedArray::new(vec![1i32, 2, 3], Types::I32Array));
        let mut ptr = buffer.leak();
        unsafe {
            ptr.release();
        }
        logger::sync().unwrap();
    }
}
