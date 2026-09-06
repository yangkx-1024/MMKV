use crate::Error::{DataInvalid, DecodeFailed};
use crate::Result;
use crate::core::buffer::{Buffer, DecodeResult, Decoder, Encoder, encode_kv_bytes, split_frame};
use crc::{CRC_8_AUTOSAR, Crc};

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
        let (frame, read_len) = split_frame(data)?;
        // Frame layout: [kv proto bytes][1-byte crc]. An empty frame is never written by
        // the encoder, so treat it as corruption rather than trusting what follows it.
        let (sum, bytes_to_decode) = frame.split_last().ok_or(DataInvalid)?;
        let result = if CRC8.checksum(bytes_to_decode) == *sum {
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

/// Unit tests for the CRC-8 record framing used by the default (unencrypted) build.
#[cfg(test)]
mod tests {
    use crate::core::buffer::{Buffer, Decoder, Encoder};
    use crate::core::crc::CrcEncoder;

    #[test]
    fn encode_and_decode_roundtrip_a_record() {
        let buffer = Buffer::new("key", 1i32);
        let bytes = CrcEncoder
            .encode_to_bytes("key", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();
        let decode_result = CrcEncoder.decode_bytes(bytes.as_slice(), 0).unwrap();
        assert_eq!(decode_result.len, bytes.len() as u32);
        assert_eq!(decode_result.buffer, Some(buffer));
    }

    #[test]
    fn decode_rejects_malformed_frames() {
        use crate::Error::DataInvalid;
        let malformed: [&[u8]; 4] = [
            &[],
            &[0, 1],
            // zero-length frame: no room for the crc byte
            &[0, 0, 0, 0],
            // declared length runs past the input
            &[0, 0, 0x03, 0xE8, 1, 2, 3, 4],
        ];
        for data in malformed {
            assert_eq!(
                CrcEncoder.decode_bytes(data, 0).err(),
                Some(DataInvalid),
                "{data:?}"
            );
        }
    }

    #[test]
    fn decode_skips_a_record_with_a_bad_crc() {
        let buffer = Buffer::new("key", 1i32);
        let mut bytes = CrcEncoder
            .encode_to_bytes("key", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let result = CrcEncoder.decode_bytes(&bytes, 0).unwrap();
        assert!(result.buffer.is_none());
        assert_eq!(result.len, bytes.len() as u32);
    }

    #[test]
    fn decode_reads_only_the_first_of_two_concatenated_records() {
        let first = Buffer::new("key1", 1i32);
        let second = Buffer::new("key2", 2i32);
        let first_bytes = CrcEncoder
            .encode_to_bytes("key1", first.kv_type(), first.kv_value(), 0)
            .unwrap();
        let second_bytes = CrcEncoder
            .encode_to_bytes("key2", second.kv_type(), second.kv_value(), 1)
            .unwrap();
        let mut stream = first_bytes.clone();
        stream.extend_from_slice(&second_bytes);

        let result = CrcEncoder.decode_bytes(&stream, 0).unwrap();
        assert_eq!(result.len, first_bytes.len() as u32);
        assert_eq!(result.buffer, Some(first));
    }
}
