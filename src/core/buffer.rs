use std::str;

use protobuf::{EnumOrUnknown, Message};

use kv::{KV, Types};

include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));

#[derive(Debug, Clone)]
pub struct Buffer(KV);

pub trait Encoder {
    fn encode_to_bytes(&self) -> Vec<u8>;
}

pub trait Decoder {
    fn decode_bytes(&mut self, data: &[u8]) -> u32;
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
        let out = if value {
            1u8
        } else {
            0u8
        };
        Buffer::from_kv(key, Types::BYTE, vec![out].as_slice())
    }

    pub fn from_encoded_bytes(data: &[u8]) -> Self {
        Buffer(KV::parse_from_bytes(data).unwrap())
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

    pub fn decode_i32(&self) -> Option<i32> {
        if self.0.type_.unwrap() != Types::I32 {
            return None;
        }
        let array_result: Result<[u8; 4], _> = self.0.value[0..4].try_into();
        array_result.ok().map(|value| {
            i32::from_be_bytes(value)
        })
    }

    pub fn decode_str(&self) -> Option<&str> {
        match self.0.type_.enum_value() {
            Ok(Types::STR) => {
                str::from_utf8(self.0.value.as_slice()).ok()
            }
            _ => None
        }
    }

    pub fn decode_bool(&self) -> Option<bool> {
        match self.0.type_.enum_value() {
            Ok(Types::BYTE) => {
                Some(self.0.value[0] == 1)
            }
            _ => None
        }
    }
}

impl PartialEq for Buffer {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(test)]
mod tests {
    use crate::core::buffer::Buffer;

    #[test]
    fn test_buffer() {
        let buffer = Buffer::from_str("first_key", "first_value");
        let bytes = buffer.to_bytes();
        let copy = Buffer::from_encoded_bytes(bytes.as_slice());
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_str().unwrap(), "first_value");
        assert_eq!(copy.decode_i32(), None);
        assert_eq!(copy.decode_bool(), None);

        let buffer = Buffer::from_i32("first_key", 1);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice());
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_str(), None);
        assert_eq!(copy.decode_i32(), Some(1));
        assert_eq!(copy.decode_bool(), None);

        let buffer = Buffer::from_bool("first_key", true);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice());
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_str(), None);
        assert_eq!(copy.decode_i32(), None);
        assert_eq!(copy.decode_bool(), Some(true));
    }
}