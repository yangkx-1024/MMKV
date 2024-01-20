use crate::core::buffer::{Buffer, DecodeResult, Decoder, Encoder};
use crate::Error::{DataInvalid, DecodeFailed};
use crate::Result;
use crc::{Crc, CRC_8_AUTOSAR};
use std::mem::size_of;

const LOG_TAG: &str = "MMKV:Crc";

const CRC8: Crc<u8> = Crc::<u8>::new(&CRC_8_AUTOSAR);

pub struct CrcEncoderDecoder;

impl Encoder for CrcEncoderDecoder {
    fn encode_to_bytes(&self, raw_buffer: &Buffer, _: u32) -> Result<Vec<u8>> {
        let bytes_to_write = raw_buffer.to_bytes();
        let sum = CRC8.checksum(bytes_to_write.as_slice());
        let len = bytes_to_write.len() as u32 + 1;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(bytes_to_write.as_slice());
        data.push(sum);
        Ok(data)
    }
}

impl Decoder for CrcEncoderDecoder {
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
    use crate::core::crc::CrcEncoderDecoder;

    #[test]
    fn test_crc_buffer() {
        let buffer = Buffer::from_i32("key", 1);
        let bytes = CrcEncoderDecoder.encode_to_bytes(&buffer, 0).unwrap();
        let decode_result = CrcEncoderDecoder.decode_bytes(bytes.as_slice(), 0).unwrap();
        assert_eq!(decode_result.len, bytes.len() as u32);
        assert_eq!(decode_result.buffer, Some(buffer));
    }
}
