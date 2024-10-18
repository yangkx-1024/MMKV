use std::mem::size_of;
use std::{f32, f64, str, vec};

use crate::Error::{DataInvalid, DecodeFailed, KeyNotFound, TypeMissMatch};
use crate::Result;
use kv::{Types, KV};
use protobuf::{EnumOrUnknown, Message};

include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct Buffer(KV);

pub trait Encoder: Send + Sync {
    fn encode_to_bytes(&self, raw_buffer: &Buffer, position: u32) -> Result<Vec<u8>>;
}

pub struct DecodeResult {
    pub buffer: Option<Buffer>,
    pub len: u32,
}

pub trait Decoder: Send + Sync {
    fn decode_bytes(&self, data: &[u8], position: u32) -> Result<DecodeResult>;
}

impl Buffer {
    fn from_kv(key: &str, t: Types, value: &[u8]) -> Self {
        let mut kv = KV::new();
        kv.key = key.to_string();
        kv.type_ = EnumOrUnknown::new(t);
        kv.value = value.to_vec();
        Buffer(kv)
    }

    pub fn encode<T: ToBuffer>(key: &str, value: T) -> Self {
        value.to_buffer(key)
    }

    pub fn decode<T: FromBuffer>(&self) -> Result<T> {
        T::from_buffer(self)
    }

    pub fn deleted_buffer(key: &str) -> Self {
        Buffer::from_kv(key, Types::DELETED, vec![].as_slice())
    }

    pub fn from_encoded_bytes(data: &[u8]) -> Result<Self> {
        let kv = KV::parse_from_bytes(data).map_err(|e| DecodeFailed(e.to_string()))?;
        Ok(Buffer(kv))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.write_to_bytes().unwrap()
    }

    pub fn key(&self) -> &str {
        self.0.key.as_str()
    }

    #[allow(dead_code)]
    pub fn value(&self) -> &[u8] {
        self.0.value.as_slice()
    }

    pub fn is_deleting(&self) -> bool {
        if let Ok(buffer_type) = self.0.type_.enum_value() {
            buffer_type == Types::DELETED
        } else {
            false
        }
    }

    fn check_buffer_type(&self, required: Types) -> Result<()> {
        if self.is_deleting() {
            return Err(KeyNotFound);
        }
        if required == self.0.type_.enum_value().map_err(|_| TypeMissMatch)? {
            Ok(())
        } else {
            Err(TypeMissMatch)
        }
    }
}

pub trait ToBuffer {
    fn to_buffer(self, key: &str) -> Buffer;
}

impl ToBuffer for &str {
    fn to_buffer(self, key: &str) -> Buffer {
        Buffer::from_kv(key, Types::STR, self.as_bytes())
    }
}

impl ToBuffer for bool {
    fn to_buffer(self, key: &str) -> Buffer {
        let out = if self { 1u8 } else { 0u8 };
        Buffer::from_kv(key, Types::BYTE, vec![out].as_slice())
    }
}

impl ToBuffer for &[u8] {
    fn to_buffer(self, key: &str) -> Buffer {
        Buffer::from_kv(key, Types::BYTE_ARRAY, self)
    }
}

macro_rules! impl_to_buffer_for_number {
    ($(($t:ty, $kv_type:expr)),+) => {
        $(
        impl ToBuffer for $t {
            fn to_buffer(self, key: &str) -> Buffer {
                Buffer::from_kv(key, $kv_type, self.to_be_bytes().as_slice())
            }
        }
        )+
    };
}

impl_to_buffer_for_number!(
    (i32, Types::I32),
    (i64, Types::I64),
    (f32, Types::F32),
    (f64, Types::F64)
);

macro_rules! impl_to_buffer_for_typed_array {
    ($(($t:ty, $kv_type:expr)),+) => {
        $(
        impl ToBuffer for &[$t] {
            fn to_buffer(self, key: &str) -> Buffer {
                let mut vec = Vec::with_capacity(self.len() * (size_of::<$t>() / size_of::<u8>()));
                for item in self {
                    vec.extend_from_slice(item.to_be_bytes().as_slice());
                }
                Buffer::from_kv(key, $kv_type, vec.as_slice())
            }
        }
        )+
    };
}

impl_to_buffer_for_typed_array!(
    (i32, Types::I32),
    (i64, Types::I64),
    (f32, Types::F32),
    (f64, Types::F64)
);

pub trait FromBuffer {
    fn from_buffer(buffer: &Buffer) -> Result<Self>
    where
        Self: Sized;
}

impl FromBuffer for String {
    fn from_buffer(buffer: &Buffer) -> Result<Self> {
        buffer.check_buffer_type(Types::STR)?;
        if let Ok(str) = String::from_utf8(buffer.0.value.to_vec()) {
            Ok(str)
        } else {
            Err(DataInvalid)
        }
    }
}

impl FromBuffer for bool {
    fn from_buffer(buffer: &Buffer) -> Result<Self> {
        buffer.check_buffer_type(Types::BYTE)?;
        Ok(buffer.0.value[0] == 1)
    }
}

impl FromBuffer for Vec<u8> {
    fn from_buffer(buffer: &Buffer) -> Result<Self> {
        buffer.check_buffer_type(Types::BYTE_ARRAY)?;
        Ok(buffer.0.value.to_vec())
    }
}

macro_rules! impl_from_buffer_for_number {
    ($(($t:ty, $kv_type:expr)),+) => {
        $(
        impl FromBuffer for $t {
            fn from_buffer(buffer: &Buffer) -> Result<Self> {
                buffer.check_buffer_type($kv_type)?;
                const ITEM_SIZE: usize = size_of::<$t>() / size_of::<u8>();
                let array_result: std::result::Result<[u8; ITEM_SIZE], _> =
                    buffer.0.value[0..ITEM_SIZE].try_into();
                match array_result {
                    Ok(array) => Ok(<$t>::from_be_bytes(array)),
                    Err(_) => Err(DataInvalid),
                }
            }
        }
        )+
    };
}

impl_from_buffer_for_number!(
    (i32, Types::I32),
    (i64, Types::I64),
    (f32, Types::F32),
    (f64, Types::F64)
);

macro_rules! impl_from_buffer_for_typed_array {
    ($(($t:ty, $kv_type:expr)),+) => {
        $(
        impl FromBuffer for Vec<$t> {
            fn from_buffer(buffer: &Buffer) -> Result<Self> {
                buffer.check_buffer_type($kv_type)?;
                const ITEM_SIZE: usize = size_of::<$t>() / size_of::<u8>();
                if buffer.0.value.len() % ITEM_SIZE != 0 {
                    return Err(DataInvalid);
                }
                let len = buffer.0.value.len() / ITEM_SIZE;
                let mut vec = Vec::with_capacity(len);
                for i in 0..len {
                    let sub_arr: [u8; ITEM_SIZE] = buffer.0.value[i * ITEM_SIZE..(i + 1) * ITEM_SIZE]
                        .try_into()
                        .unwrap();
                    let value = <$t>::from_be_bytes(sub_arr);
                    vec.push(value)
                }
                Ok(vec)
            }
        }
        )+
    };
}

impl_from_buffer_for_typed_array!(
    (i32, Types::I32),
    (i64, Types::I64),
    (f32, Types::F32),
    (f64, Types::F64)
);

impl PartialEq for Buffer {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(test)]
mod tests {
    use crate::core::buffer::{Buffer, TypeMissMatch};

    #[test]
    fn test_buffer() {
        let buffer = Buffer::encode("first_key", "first_value");
        let bytes = buffer.to_bytes();
        let copy = Buffer::from_encoded_bytes(bytes.as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode(), Ok("first_value".to_string()));
        assert_eq!(copy.decode::<i32>(), Err(TypeMissMatch));
        assert_eq!(copy.decode::<bool>(), Err(TypeMissMatch));

        let buffer = Buffer::encode("first_key", i32::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode::<String>(), Err(TypeMissMatch));
        assert_eq!(copy.decode(), Ok(i32::MAX));
        assert_eq!(copy.decode::<bool>(), Err(TypeMissMatch));

        let buffer = Buffer::encode("first_key", true);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode::<String>(), Err(TypeMissMatch));
        assert_eq!(copy.decode::<i32>(), Err(TypeMissMatch));
        assert_eq!(copy.decode(), Ok(true));

        let buffer = Buffer::encode("first_key", i64::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode(), Ok(i64::MAX));
        assert_eq!(copy.decode::<i32>(), Err(TypeMissMatch));

        let buffer = Buffer::encode("first_key", f32::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode(), Ok(f32::MAX));
        assert_eq!(copy.decode::<i32>(), Err(TypeMissMatch));

        let buffer = Buffer::encode("first_key", f64::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode(), Ok(f64::MAX));
        assert_eq!(copy.decode::<f32>(), Err(TypeMissMatch));

        let byte_array = vec![u8::MIN, 2, u8::MAX];
        let buffer = Buffer::encode("byte_array", byte_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode(), Ok(byte_array));
        assert_eq!(copy.decode::<Vec<i32>>(), Err(TypeMissMatch));

        let i32_array = vec![i32::MIN, 2, i32::MAX];
        let buffer = Buffer::encode("i32_array", i32_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode(), Ok(i32_array));
        assert_eq!(copy.decode::<Vec<i64>>(), Err(TypeMissMatch));

        let i64_array = vec![i64::MIN, 2, i64::MAX];
        let buffer = Buffer::encode("i64_array", i64_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode(), Ok(i64_array));
        assert_eq!(copy.decode::<Vec<i32>>(), Err(TypeMissMatch));

        let f32_array = vec![f32::MIN, 2.2, f32::MAX];
        let buffer = Buffer::encode("f32_array", f32_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode(), Ok(f32_array));
        assert_eq!(copy.decode::<Vec<f64>>(), Err(TypeMissMatch));

        let f64_array = vec![f64::MIN, 2.2, f64::MAX];
        let buffer = Buffer::encode("f64_array", f64_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode(), Ok(f64_array));
        assert_eq!(copy.decode::<Vec<u8>>(), Err(TypeMissMatch));
    }
}
