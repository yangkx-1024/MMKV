include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
use protobuf::Message;
use kv::KV;

#[derive(Debug)]
pub struct Buffer {
    raw_data: KV,
    len: u32,
}

impl Buffer {
    pub fn from_kv(key: &str, value: &[u8]) -> Self {
        let mut kv = KV::new();
        kv.key = key.to_string();
        kv.value = value.to_vec();
        Buffer {
            raw_data: kv,
            len: 0
        }
    }

    pub fn from_encoded_bytes(data: &[u8]) -> Self {
        let item_len = u32::from_be_bytes(
            data[0..4].try_into().unwrap()
        );
        let kv = KV::parse_from_bytes(&data[4..(4 + item_len as usize)]).unwrap();
        Buffer {
            raw_data: kv,
            len: item_len + 4,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let bytes_to_write = self.raw_data.write_to_bytes().unwrap();
        let len = bytes_to_write.len() as u32;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(bytes_to_write.as_slice());
        data
    }

    pub fn decode_i32(&self) -> Option<i32> {
        let array_result: Result<[u8; 4], _> = self.raw_data.value[0..4].try_into();
        array_result.ok().map(|value| {
            i32::from_be_bytes(value)
        })
    }

    pub fn key(&self) -> &str {
        self.raw_data.key.as_str()
    }

    #[allow(dead_code)]
    pub fn value(&self) -> &[u8] {
        self.raw_data.value.as_slice()
    }

    pub fn len(&self) -> u32 {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use crate::core::buffer::Buffer;

    #[test]
    fn test_buffer() {
        let buffer = Buffer::from_kv("first_key", "first_value".as_bytes());
        assert_eq!(buffer.key(), "first_key");
        assert_eq!(buffer.value(), "first_value".as_bytes());
        let bytes = buffer.to_bytes();
        let copy = Buffer::from_encoded_bytes(bytes.as_slice());
        assert_eq!(copy.raw_data, buffer.raw_data);
    }
}