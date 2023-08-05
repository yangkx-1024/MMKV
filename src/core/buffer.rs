use std::str;

use protobuf::{EnumOrUnknown, Message};

use kv::{KV, Types};
use crate::core::crc::CRC8;

include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
#[derive(Debug)]
pub struct Buffer(KV);

pub enum BufferResult {
    Ok(Buffer),
    Invalid()
}

impl BufferResult {
    #[allow(dead_code)]
    pub fn unwrap(self) -> Buffer {
        match self {
            BufferResult::Ok(buffer) => buffer,
            BufferResult::Invalid() => panic!("Invalid buffer data"),
        }
    }
}

pub trait Decoder {
    fn decode_i32(&self) -> Option<i32>;
    fn decode_str(&self) -> Option<&str>;
    fn decode_bool(&self) -> Option<bool>;
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

    pub fn from_encoded_bytes(data: &[u8]) -> (BufferResult, u32) {
        let item_len = u32::from_be_bytes(
            data[0..4].try_into().unwrap()
        );
        let bytes_to_decode = &data[4..(3 + item_len as usize)];
        let sum = data[3 + item_len as usize];
        let result = if CRC8.checksum(bytes_to_decode) == sum {
            BufferResult::Ok(
                Buffer(KV::parse_from_bytes(bytes_to_decode).unwrap()),
            )
        } else {
            BufferResult::Invalid()
        };
        (result, 4 + item_len)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let bytes_to_write = self.0.write_to_bytes().unwrap();
        let sum = CRC8.checksum(bytes_to_write.as_slice());
        let len = bytes_to_write.len() as u32 + 1;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(bytes_to_write.as_slice());
        data.push(sum);
        data
    }

    pub fn key(&self) -> &str {
        self.0.key.as_str()
    }

    #[allow(dead_code)]
    pub fn value(&self) -> &[u8] {
        self.0.value.as_slice()
    }
}

impl Decoder for Buffer {
    fn decode_i32(&self) -> Option<i32> {
        if self.0.type_.unwrap() != Types::I32 {
            return None;
        }
        let array_result: Result<[u8; 4], _> = self.0.value[0..4].try_into();
        array_result.ok().map(|value| {
            i32::from_be_bytes(value)
        })
    }

    fn decode_str(&self) -> Option<&str> {
        match self.0.type_.enum_value() {
            Ok(Types::STR) => {
                str::from_utf8(self.0.value.as_slice()).ok()
            }
            _ => None
        }
    }

    fn decode_bool(&self) -> Option<bool> {
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

    fn ne(&self, other: &Self) -> bool {
        self.0 != other.0
    }
}

#[cfg(test)]
mod tests {
    use crate::core::buffer::{Buffer, Decoder};

    #[test]
    fn test_buffer() {
        let buffer = Buffer::from_str("first_key", "first_value");
        let bytes = buffer.to_bytes();
        let data_len = bytes.len();
        let (copy, len) = Buffer::from_encoded_bytes(bytes.as_slice());
        let copy = copy.unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(len , data_len as u32);
        assert_eq!(copy.decode_str().unwrap(), "first_value");
        assert_eq!(copy.decode_i32(), None);
        assert_eq!(copy.decode_bool(), None);

        let buffer = Buffer::from_i32("first_key", 1);
        let (copy, _) = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice());
        let copy = copy.unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_str(), None);
        assert_eq!(copy.decode_i32(), Some(1));
        assert_eq!(copy.decode_bool(), None);

        let buffer = Buffer::from_bool("first_key", true);
        let (copy, _) = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice());
        let copy = copy.unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.decode_str(), None);
        assert_eq!(copy.decode_i32(), None);
        assert_eq!(copy.decode_bool(), Some(true));
    }
}