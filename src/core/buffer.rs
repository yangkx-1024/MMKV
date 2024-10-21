use std::mem::size_of;
use std::{f32, f64, str, vec};

use crate::Error::{DataInvalid, DecodeFailed, KeyNotFound, TypeMissMatch};
use crate::Result;
use kv::KV;
use protobuf::Message;

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
    fn from_kv(key: &str, t: i32, value: Vec<u8>) -> Self {
        let mut kv = KV::new();
        kv.key = key.to_string();
        kv.type_ = t;
        kv.value = value;
        Buffer(kv)
    }

    pub fn new<T: ProvideTypeToken + ToBytes>(key: &str, value: T) -> Self {
        Buffer::from_kv(key, T::type_token().token, value.to_bytes())
    }

    pub fn parse<T: ProvideTypeToken + FromBytes>(&self) -> Result<T> {
        self.check_buffer_type(T::type_token())?;
        T::from_bytes(self.0.value.as_slice())
    }

    pub fn deleted_buffer(key: &str) -> Self {
        Buffer::from_kv(key, InnerTypes::Deleted.value(), vec![])
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
        self.0.type_ == InnerTypes::Deleted.value()
    }

    fn check_buffer_type(&self, type_token: TypeToken) -> Result<()> {
        if self.is_deleting() {
            return Err(KeyNotFound);
        }
        if type_token.token == self.0.type_ {
            Ok(())
        } else {
            Err(TypeMissMatch)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum InnerTypes {
    I32 = 0,
    Str = 1,
    Byte = 2,
    I64 = 3,
    F32 = 4,
    F64 = 5,
    ByteArray = 6,
    I32Array = 7,
    I64Array = 8,
    F32Array = 9,
    F64Array = 10,
    Deleted = 100,
}

impl InnerTypes {
    fn value(&self) -> i32 {
        *self as i32
    }

    fn reserved(value: i32) -> bool {
        (0..=100).contains(&value)
    }
}

/// 0 ~ 100 reserved for internal usage.
pub struct TypeToken {
    token: i32,
}

impl TypeToken {
    /// Provide an int for type token, 0 ~ 100 reserved for internal usage.
    ///
    /// Notice: Panic when token is in 0 ~ 100
    pub fn new(token: i32) -> Self {
        if InnerTypes::reserved(token) {
            panic!("type token 0 ~ 100 reserved for internal usage");
        }
        TypeToken { token }
    }

    fn from_int_unchecked(token: i32) -> Self {
        TypeToken { token }
    }
}

/// See [crate::MMKV::put]
pub trait ToBytes {
    /// Serialize to bytes
    fn to_bytes(&self) -> Vec<u8>;
}

impl<T> ToBytes for &T
where
    T: ToBytes,
{
    fn to_bytes(&self) -> Vec<u8> {
        (*self).to_bytes()
    }
}

/// See [crate::MMKV::put]
pub trait ProvideTypeToken {
    /// See [TypeToken::new]
    fn type_token() -> TypeToken;
}

impl<T> ProvideTypeToken for &T
where
    T: ProvideTypeToken,
{
    fn type_token() -> TypeToken {
        T::type_token()
    }
}

impl ProvideTypeToken for &str {
    fn type_token() -> TypeToken {
        TypeToken::from_int_unchecked(InnerTypes::Str.value())
    }
}

impl ProvideTypeToken for String {
    fn type_token() -> TypeToken {
        TypeToken::from_int_unchecked(InnerTypes::Str.value())
    }
}

impl ToBytes for &str {
    fn to_bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl ToBytes for String {
    fn to_bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl ProvideTypeToken for bool {
    fn type_token() -> TypeToken {
        TypeToken::from_int_unchecked(InnerTypes::Byte.value())
    }
}

impl ToBytes for bool {
    fn to_bytes(&self) -> Vec<u8> {
        let out = if *self { 1u8 } else { 0u8 };
        vec![out]
    }
}

impl ProvideTypeToken for &[u8] {
    fn type_token() -> TypeToken {
        TypeToken::from_int_unchecked(InnerTypes::ByteArray.value())
    }
}

impl ProvideTypeToken for Vec<u8> {
    fn type_token() -> TypeToken {
        TypeToken::from_int_unchecked(InnerTypes::ByteArray.value())
    }
}

impl ToBytes for &[u8] {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_vec()
    }
}
macro_rules! impl_to_bytes_for_number {
    ($(($t:ty, $kv_type:expr)),+;) => {
        $(
        impl ProvideTypeToken for $t {
            fn type_token() -> TypeToken {
                TypeToken::from_int_unchecked($kv_type.value())
            }
        }
        impl ToBytes for $t {
            fn to_bytes(&self) -> Vec<u8> {
                self.to_be_bytes().to_vec()
            }
        }
        )+
    };
}

impl_to_bytes_for_number!(
    (i32, InnerTypes::I32),
    (i64, InnerTypes::I64),
    (f32, InnerTypes::F32),
    (f64, InnerTypes::F64);
);

macro_rules! impl_to_bytes_for_typed_array {
    ($(($t:ty, $kv_type:expr)),+;) => {
        $(
        impl ProvideTypeToken for &[$t] {
            fn type_token() -> TypeToken {
                TypeToken::from_int_unchecked($kv_type.value())
            }
        }
        impl ProvideTypeToken for Vec<$t> {
            fn type_token() -> TypeToken {
                TypeToken::from_int_unchecked($kv_type.value())
            }
        }
        impl ToBytes for &[$t] {
            fn to_bytes(&self) -> Vec<u8> {
                let mut vec = Vec::with_capacity(self.len() * (size_of::<$t>() / size_of::<u8>()));
                for item in *self {
                    vec.extend_from_slice(item.to_be_bytes().as_slice());
                }
                vec
            }
        }
        )+
    };
}

impl_to_bytes_for_typed_array!(
    (i32, InnerTypes::I32Array),
    (i64, InnerTypes::I64Array),
    (f32, InnerTypes::F32Array),
    (f64, InnerTypes::F64Array);
);

/// See [crate::MMKV::put]
pub trait FromBytes {
    /// Deserialize from bytes
    fn from_bytes(bytes: &[u8]) -> Result<Self>
    where
        Self: Sized;
}

impl FromBytes for String {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        String::from_utf8(bytes.to_vec()).map_err(|_| DataInvalid)
    }
}

impl FromBytes for bool {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(bytes[0] == 1)
    }
}

impl FromBytes for Vec<u8> {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(bytes.to_vec())
    }
}

macro_rules! impl_from_buffer_for_number {
    ($(($t:ty, $kv_type:expr)),+;) => {
        $(
        impl FromBytes for $t {
            fn from_bytes(bytes: &[u8]) -> Result<Self> {
                const ITEM_SIZE: usize = size_of::<$t>() / size_of::<u8>();
                let array_result: std::result::Result<[u8; ITEM_SIZE], _> =
                    bytes[0..ITEM_SIZE].try_into();
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
    (f64, Types::F64);
);

macro_rules! impl_from_buffer_for_typed_array {
    ($(($t:ty, $kv_type:expr)),+;) => {
        $(
        impl FromBytes for Vec<$t> {
            fn from_bytes(bytes: &[u8]) -> Result<Self> {
                const ITEM_SIZE: usize = size_of::<$t>() / size_of::<u8>();
                if bytes.len() % ITEM_SIZE != 0 {
                    return Err(DataInvalid);
                }
                let len = bytes.len() / ITEM_SIZE;
                let mut vec = Vec::with_capacity(len);
                for i in 0..len {
                    let sub_arr: [u8; ITEM_SIZE] = bytes[i * ITEM_SIZE..(i + 1) * ITEM_SIZE]
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
    (i32, Types::I32_ARRAY),
    (i64, Types::I64_ARRAY),
    (f32, Types::F32_ARRAY),
    (f64, Types::F64_ARRAY);
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
        let buffer = Buffer::new("first_key", "first_value");
        let bytes = buffer.to_bytes();
        let copy = Buffer::from_encoded_bytes(bytes.as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(), Ok("first_value".to_string()));
        assert_eq!(copy.parse::<i32>(), Err(TypeMissMatch));
        assert_eq!(copy.parse::<bool>(), Err(TypeMissMatch));

        let buffer = Buffer::new("first_key", i32::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse::<String>(), Err(TypeMissMatch));
        assert_eq!(copy.parse(), Ok(i32::MAX));
        assert_eq!(copy.parse::<bool>(), Err(TypeMissMatch));

        let buffer = Buffer::new("first_key", true);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse::<String>(), Err(TypeMissMatch));
        assert_eq!(copy.parse::<i32>(), Err(TypeMissMatch));
        assert_eq!(copy.parse(), Ok(true));

        let buffer = Buffer::new("first_key", i64::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(), Ok(i64::MAX));
        assert_eq!(copy.parse::<i32>(), Err(TypeMissMatch));

        let buffer = Buffer::new("first_key", f32::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(), Ok(f32::MAX));
        assert_eq!(copy.parse::<i32>(), Err(TypeMissMatch));

        let buffer = Buffer::new("first_key", f64::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(), Ok(f64::MAX));
        assert_eq!(copy.parse::<f32>(), Err(TypeMissMatch));

        let byte_array = vec![u8::MIN, 2, u8::MAX];
        let buffer = Buffer::new("byte_array", byte_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(), Ok(byte_array));
        assert_eq!(copy.parse::<Vec<i32>>(), Err(TypeMissMatch));

        let i32_array = vec![i32::MIN, 2, i32::MAX];
        let buffer = Buffer::new("i32_array", i32_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(), Ok(i32_array));
        assert_eq!(copy.parse::<Vec<i64>>(), Err(TypeMissMatch));

        let i64_array = vec![i64::MIN, 2, i64::MAX];
        let buffer = Buffer::new("i64_array", i64_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(), Ok(i64_array));
        assert_eq!(copy.parse::<Vec<i32>>(), Err(TypeMissMatch));

        let f32_array = vec![f32::MIN, 2.2, f32::MAX];
        let buffer = Buffer::new("f32_array", f32_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(), Ok(f32_array));
        assert_eq!(copy.parse::<Vec<f64>>(), Err(TypeMissMatch));

        let f64_array = vec![f64::MIN, 2.2, f64::MAX];
        let buffer = Buffer::new("f64_array", f64_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(), Ok(f64_array));
        assert_eq!(copy.parse::<Vec<u8>>(), Err(TypeMissMatch));
    }
}
