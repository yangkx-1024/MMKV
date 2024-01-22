use std::mem::size_of;
use std::{f32, f64, str};

use protobuf::{EnumOrUnknown, Message};

use crate::Error::{DataInvalid, DecodeFailed, KeyNotFound, TypeMissMatch};
use crate::Result;
use kv::{Types, KV};

include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct Buffer(KV);

pub trait Encoder: Send {
    fn encode_to_bytes(&self, raw_buffer: &Buffer, position: u32) -> Result<Vec<u8>>;
}

pub struct DecodeResult {
    pub buffer: Option<Buffer>,
    pub len: u32,
}

pub trait Decoder: Send {
    fn decode_bytes(&self, data: &[u8], position: u32) -> Result<DecodeResult>;
}

macro_rules! impl_from_typed_array {
    ($name:ident, $t:ty, $kv_type:expr) => {
        pub fn $name(key: &str, value: &[$t]) -> Self {
            let mut vec = Vec::with_capacity(value.len() * (size_of::<$t>() / size_of::<u8>()));
            for item in value {
                vec.extend_from_slice(item.to_be_bytes().as_slice());
            }
            Buffer::from_kv(key, $kv_type, vec.as_slice())
        }
    };
}

macro_rules! impl_from_number {
    ($name:ident, $t:ty, $kv_type:expr) => {
        pub fn $name(key: &str, value: $t) -> Self {
            Buffer::from_kv(key, $kv_type, value.to_be_bytes().as_ref())
        }
    };
}

macro_rules! impl_decode_number {
    ($name:ident, $t:ty, $kv_type:expr) => {
        pub fn $name(&self) -> Result<$t> {
            self.check_buffer_type($kv_type)?;
            const ITEM_SIZE: usize = size_of::<$t>() / size_of::<u8>();
            let array_result: std::result::Result<[u8; ITEM_SIZE], _> =
                self.0.value[0..ITEM_SIZE].try_into();
            match array_result {
                Ok(array) => Ok(<$t>::from_be_bytes(array)),
                Err(_) => Err(DataInvalid),
            }
        }
    };
}

macro_rules! impl_decode_typed_array {
    ($name:ident, $t:ty, $kv_type:expr) => {
        pub fn $name(&self) -> Result<Vec<$t>> {
            self.check_buffer_type($kv_type)?;
            const ITEM_SIZE: usize = size_of::<$t>() / size_of::<u8>();
            if self.0.value.len() % ITEM_SIZE != 0 {
                return Err(DataInvalid);
            }
            let len = self.0.value.len() / ITEM_SIZE;
            let mut vec = Vec::with_capacity(len);
            for i in 0..len {
                let sub_arr: [u8; ITEM_SIZE] = self.0.value[i * ITEM_SIZE..(i + 1) * ITEM_SIZE]
                    .try_into()
                    .unwrap();
                let value = <$t>::from_be_bytes(sub_arr);
                vec.push(value)
            }
            Ok(vec)
        }
    };
}

impl Buffer {
    fn from_kv(key: &str, t: Types, value: &[u8]) -> Self {
        let mut kv = KV::new();
        kv.key = key.to_string();
        kv.type_ = EnumOrUnknown::new(t);
        kv.value = value.to_vec();
        Buffer(kv)
    }

    impl_from_number!(from_i32, i32, Types::I32);

    impl_from_number!(from_i64, i64, Types::I64);

    impl_from_number!(from_f32, f32, Types::F32);

    impl_from_number!(from_f64, f64, Types::F64);

    pub fn deleted_buffer(key: &str) -> Self {
        Buffer::from_kv(key, Types::DELETED, vec![].as_slice())
    }

    pub fn from_str(key: &str, value: &str) -> Self {
        Buffer::from_kv(key, Types::STR, value.as_bytes())
    }

    pub fn from_bool(key: &str, value: bool) -> Self {
        let out = if value { 1u8 } else { 0u8 };
        Buffer::from_kv(key, Types::BYTE, vec![out].as_slice())
    }

    pub fn from_byte_array(key: &str, value: &[u8]) -> Self {
        Buffer::from_kv(key, Types::BYTE_ARRAY, value)
    }

    impl_from_typed_array!(from_i32_array, i32, Types::I32_ARRAY);

    impl_from_typed_array!(from_i64_array, i64, Types::I64_ARRAY);

    impl_from_typed_array!(from_f32_array, f32, Types::F32_ARRAY);

    impl_from_typed_array!(from_f64_array, f64, Types::F64_ARRAY);

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

    impl_decode_number!(decode_i32, i32, Types::I32);

    impl_decode_number!(decode_i64, i64, Types::I64);

    impl_decode_number!(decode_f32, f32, Types::F32);

    impl_decode_number!(decode_f64, f64, Types::F64);

    pub fn decode_str(&self) -> Result<String> {
        self.check_buffer_type(Types::STR)?;
        if let Ok(str) = String::from_utf8(self.0.value.to_vec()) {
            Ok(str)
        } else {
            Err(DataInvalid)
        }
    }

    pub fn decode_bool(&self) -> Result<bool> {
        self.check_buffer_type(Types::BYTE)?;
        Ok(self.0.value[0] == 1)
    }

    pub fn decode_byte_array(&self) -> Result<Vec<u8>> {
        self.check_buffer_type(Types::BYTE_ARRAY)?;
        Ok(self.0.value.to_vec())
    }

    impl_decode_typed_array!(decode_i32_array, i32, Types::I32_ARRAY);

    impl_decode_typed_array!(decode_i64_array, i64, Types::I64_ARRAY);

    impl_decode_typed_array!(decode_f32_array, f32, Types::F32_ARRAY);

    impl_decode_typed_array!(decode_f64_array, f64, Types::F64_ARRAY);
}

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
        let buffer = Buffer::from_str("first_key", "first_value");
        let bytes = buffer.to_bytes();
        let copy = Buffer::from_encoded_bytes(bytes.as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_str().unwrap(), "first_value");
        assert_eq!(copy.decode_i32(), Err(TypeMissMatch));
        assert_eq!(copy.decode_bool(), Err(TypeMissMatch));

        let buffer = Buffer::from_i32("first_key", i32::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_str(), Err(TypeMissMatch));
        assert_eq!(copy.decode_i32(), Ok(i32::MAX));
        assert_eq!(copy.decode_bool(), Err(TypeMissMatch));

        let buffer = Buffer::from_bool("first_key", true);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_str(), Err(TypeMissMatch));
        assert_eq!(copy.decode_i32(), Err(TypeMissMatch));
        assert_eq!(copy.decode_bool(), Ok(true));

        let buffer = Buffer::from_i64("first_key", i64::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_i64(), Ok(i64::MAX));
        assert_eq!(copy.decode_i32(), Err(TypeMissMatch));

        let buffer = Buffer::from_f32("first_key", f32::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_f32(), Ok(f32::MAX));
        assert_eq!(copy.decode_i32(), Err(TypeMissMatch));

        let buffer = Buffer::from_f64("first_key", f64::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_f64(), Ok(f64::MAX));
        assert_eq!(copy.decode_f32(), Err(TypeMissMatch));

        let byte_array = vec![u8::MIN, 2, u8::MAX];
        let buffer = Buffer::from_byte_array("byte_array", byte_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_byte_array(), Ok(byte_array));
        assert_eq!(copy.decode_i32_array(), Err(TypeMissMatch));

        let i32_array = vec![i32::MIN, 2, i32::MAX];
        let buffer = Buffer::from_i32_array("i32_array", i32_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_i32_array(), Ok(i32_array));
        assert_eq!(copy.decode_i64_array(), Err(TypeMissMatch));

        let i64_array = vec![i64::MIN, 2, i64::MAX];
        let buffer = Buffer::from_i64_array("i64_array", i64_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_i64_array(), Ok(i64_array));
        assert_eq!(copy.decode_f32_array(), Err(TypeMissMatch));

        let f32_array = vec![f32::MIN, 2.2, f32::MAX];
        let buffer = Buffer::from_f32_array("f32_array", f32_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_f32_array(), Ok(f32_array));
        assert_eq!(copy.decode_f64_array(), Err(TypeMissMatch));

        let f64_array = vec![f64::MIN, 2.2, f64::MAX];
        let buffer = Buffer::from_f64_array("f64_array", f64_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_f64_array(), Ok(f64_array));
        assert_eq!(copy.decode_byte_array(), Err(TypeMissMatch));
    }
}
