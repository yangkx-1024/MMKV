use crate::Error::{DataInvalid, DecodeFailed};
use crate::Result;
use crate::core::buffer::{Buffer, DecodeResult, Decoder, Encoder, encode_kv_bytes};
use crc::{CRC_8_AUTOSAR, Crc};
use std::mem::size_of;

const LOG_TAG: &str = "MMKV:Crc";

const CRC8: Crc<u8> = Crc::<u8>::new(&CRC_8_AUTOSAR);

pub struct CrcEncoder;

impl Encoder for CrcEncoder {
    fn encode_to_bytes(
        &self,
        key: &str,
        type_token: i32,
        value: &[u8],
        _position: u32,
    ) -> Result<Vec<u8>> {
        let kv_bytes = encode_kv_bytes(key, type_token, value);
        let sum = CRC8.checksum(kv_bytes.as_slice());
        let len = kv_bytes.len() as u32 + 1;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(kv_bytes.as_slice());
        data.push(sum);
        Ok(data)
    }

    fn materialize_slice(
        &self,
        mmap: &crate::core::memory_map::MmapHandle,
        buf: &Buffer,
    ) -> Option<(i32, Vec<u8>)> {
        match buf {
            Buffer::Slice(loc) => Some((loc.type_token, mmap.read(loc.byte_range()).to_vec())),
            _ => None,
        }
    }
}

impl Decoder for CrcEncoder {
    fn decode_bytes(&self, data: &[u8], _: u32) -> Result<DecodeResult> {
        let offset = size_of::<u32>();
        let item_len = u32::from_be_bytes(data[0..offset].try_into().map_err(|_| DataInvalid)?);
        let bytes_to_decode = &data[offset..(offset + item_len as usize - 1)];
        let read_len = offset as u32 + item_len;
        let sum = data[offset + item_len as usize - 1];
        let result = if CRC8.checksum(bytes_to_decode) == sum {
            Buffer::from_encoded_bytes(bytes_to_decode)
        } else {
            Err(DecodeFailed("CRC check failed".to_string()))
        };
        let buffer = match result {
            Ok(data) => Some(data),
            Err(e) => {
                error!(LOG_TAG, "Failed to decode data, reason: {:?}", e);
                None
            }
        };
        Ok(DecodeResult {
            buffer,
            len: read_len,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::core::buffer::{Buffer, Decoder, Encoder};
    use crate::core::crc::CrcEncoder;

    #[test]
    fn test_crc_buffer() {
        let buffer = Buffer::new("key", 1i32);
        let bytes = CrcEncoder
            .encode_to_bytes("key", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();
        let decode_result = CrcEncoder.decode_bytes(bytes.as_slice(), 0).unwrap();
        assert_eq!(decode_result.len, bytes.len() as u32);
        assert_eq!(decode_result.buffer, Some(buffer));
    }
}
