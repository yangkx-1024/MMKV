use std::str;

use protobuf::{EnumOrUnknown, Message};

use crate::Error::{DataInvalid, DecodeFailed, TypeMissMatch};
use crate::Result;
use kv::{Types, KV};

include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));

#[derive(Debug, Clone)]
pub struct Buffer(KV);

pub trait Encoder {
    fn encode_to_bytes(&self) -> Result<Vec<u8>>;
}

pub trait Decoder {
    fn decode_bytes_into(&mut self, data: &[u8]) -> Result<u32>;
}

pub trait Take {
    fn take(self) -> Option<Buffer>;
}

impl Buffer {
    fn from_kv(key: &str, t: Types, value: &[u8]) -> Self {
        let mut kv = KV::new();
        kv.key = key.to_string();
        kv.type_ = EnumOrUnknown::new(t);
        kv.value = value.to_vec();
        Buffer(kv)
    }

    pub fn from_i32(key: &str, value: i32) -> Self {
        Buffer::from_kv(key, Types::I32, value.to_be_bytes().as_ref())
    }

    pub fn from_str(key: &str, value: &str) -> Self {
        Buffer::from_kv(key, Types::STR, value.as_bytes())
    }

    pub fn from_bool(key: &str, value: bool) -> Self {
        let out = if value { 1u8 } else { 0u8 };
        Buffer::from_kv(key, Types::BYTE, vec![out].as_slice())
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

    fn check_buffer_type(&self, required: Types) -> Result<()> {
        if required == self.0.type_.enum_value().map_err(|_| TypeMissMatch)? {
            Ok(())
        } else {
            Err(TypeMissMatch)
        }
    }

    pub fn decode_i32(&self) -> Result<i32> {
        self.check_buffer_type(Types::I32)?;
        let array_result: std::result::Result<[u8; 4], _> = self.0.value[0..4].try_into();
        match array_result {
            Ok(array) => Ok(i32::from_be_bytes(array)),
            Err(_) => Err(DataInvalid),
        }
    }

    pub fn decode_str(&self) -> Result<&str> {
        self.check_buffer_type(Types::STR)?;
        if let Ok(str) = str::from_utf8(self.0.value.as_slice()) {
            Ok(str)
        } else {
            Err(DataInvalid)
        }
    }

    pub fn decode_bool(&self) -> Result<bool> {
        self.check_buffer_type(Types::BYTE)?;
        Ok(self.0.value[0] == 1)
    }
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

        let buffer = Buffer::from_i32("first_key", 1);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_str(), Err(TypeMissMatch));
        assert_eq!(copy.decode_i32(), Ok(1));
        assert_eq!(copy.decode_bool(), Err(TypeMissMatch));

        let buffer = Buffer::from_bool("first_key", true);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_str(), Err(TypeMissMatch));
        assert_eq!(copy.decode_i32(), Err(TypeMissMatch));
        assert_eq!(copy.decode_bool(), Ok(true));
    }
}
