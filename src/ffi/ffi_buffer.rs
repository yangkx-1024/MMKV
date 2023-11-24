use std::any::Any;
use std::fmt::Debug;

use crate::ffi::ffi::*;
use crate::Error;

pub(super) trait Releasable {
    unsafe fn release(&mut self);
}

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
    pub(super) fn new<T: Sized>(array: Vec<T>, type_token: Types) -> Self {
        let boxed = array.into_boxed_slice();
        let ptr = boxed.as_ptr();
        let len = boxed.len();
        verbose!(LOG_TAG, "leak array {:?}", ptr);
        std::mem::forget(boxed);
        RawTypedArray {
            array: ptr as *mut _,
            type_token,
            len,
        }
    }
}

impl Releasable for RawTypedArray {
    unsafe fn release(&mut self) {
        verbose!(LOG_TAG, "release {:?}", self);
        verbose!(LOG_TAG, "release array {:?}", self.array);
        let _: Box<dyn Any> = match self.type_token {
            Types::ByteArray => Box::from_raw(
                std::slice::from_raw_parts_mut(self.array as *mut u8, self.len).as_mut_ptr(),
            ),
            Types::I32Array => Box::from_raw(
                std::slice::from_raw_parts_mut(self.array as *mut i32, self.len).as_mut_ptr(),
            ),
            Types::I64Array => Box::from_raw(
                std::slice::from_raw_parts_mut(self.array as *mut i64, self.len).as_mut_ptr(),
            ),
            Types::F32Array => Box::from_raw(
                std::slice::from_raw_parts_mut(self.array as *mut f32, self.len).as_mut_ptr(),
            ),
            Types::F64Array => Box::from_raw(
                std::slice::from_raw_parts_mut(self.array as *mut f64, self.len).as_mut_ptr(),
            ),
            _ => {
                panic!("can't match type of array")
            }
        };
    }
}

impl RawBuffer {
    pub(super) fn new(type_token: Types) -> Self {
        RawBuffer {
            rawData: std::ptr::null(),
            typeToken: type_token,
            err: std::ptr::null(),
        }
    }

    pub(super) fn set_data<T>(&mut self, data: T)
    where
        T: Sized + Debug,
    {
        let log = format!("{:?}", data);
        let ptr = Box::into_raw(Box::new(data)) as *const _;
        verbose!(LOG_TAG, "leak data {log} {:?}", ptr);
        self.rawData = ptr;
    }

    unsafe fn drop_data(&mut self) {
        if self.rawData.is_null() {
            return;
        }
        verbose!(LOG_TAG, "release data {:?}", self.rawData);
        let _: Box<dyn Any> = match self.typeToken {
            Types::I32 => Box::from_raw(self.rawData as *mut i32),
            Types::Str => Box::from_raw(self.rawData as *mut ByteSlice),
            Types::Bool => Box::from_raw(self.rawData as *mut bool),
            Types::I64 => Box::from_raw(self.rawData as *mut i64),
            Types::F32 => Box::from_raw(self.rawData as *mut f32),
            Types::F64 => Box::from_raw(self.rawData as *mut f64),
            Types::ByteArray
            | Types::I32Array
            | Types::I64Array
            | Types::F32Array
            | Types::F64Array => Box::from_raw(self.rawData as *mut RawTypedArray),
        };
    }

    pub(super) fn set_error(&mut self, e: InternalError) {
        let log = format!("{:?}", e);
        let err = Box::into_raw(Box::new(e));
        verbose!(LOG_TAG, "leak {log} {:?}", err);
        self.err = err;
    }

    unsafe fn drop_error(&mut self) {
        if !self.err.is_null() {
            let _ = Box::from_raw(self.err.cast_mut());
        }
    }

    pub(super) fn leak(self) -> *mut Self {
        let log = format!("{:?}", self);
        let ptr = Box::into_raw(Box::new(self));
        verbose!(LOG_TAG, "leak {log} {:?}", ptr);
        return ptr;
    }

    pub(super) fn from_raw(ptr: *mut RawBuffer) -> Self {
        *unsafe { Box::from_raw(ptr) }
    }
}

impl Releasable for RawBuffer {
    unsafe fn release(&mut self) {
        verbose!(LOG_TAG, "release {:?}", self);
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
                let reason = Box::into_raw(Box::new(byte_slice));
                verbose!(LOG_TAG, "leak {log} {:?}", reason);
                InternalError { code, reason }
            }
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
            _ => Err(()),
        }
    }
}

impl Releasable for InternalError {
    unsafe fn release(&mut self) {
        verbose!(LOG_TAG, "release {:?}", self);
        if !self.reason.is_null() {
            let byte_slice = Box::from_raw(self.reason.cast_mut());
            verbose!(LOG_TAG, "release {:?} {:?}", byte_slice, self.reason);
        }
    }
}
