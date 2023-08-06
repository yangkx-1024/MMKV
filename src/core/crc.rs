use crc::{Crc, CRC_8_AUTOSAR};
use crate::core::buffer::{Buffer, Decoder, Encoder, Take};

const CRC8: Crc<u8> = Crc::<u8>::new(&CRC_8_AUTOSAR);

#[derive(Debug)]
pub struct CrcBuffer(Option<Buffer>);

impl CrcBuffer {
    pub fn new() -> Self {
        CrcBuffer(None)
    }

    pub fn new_with_buffer(buffer: Buffer) -> Self {
        CrcBuffer(Some(buffer))
    }
}

impl Encoder for CrcBuffer {
    fn encode_to_bytes(&self) -> Vec<u8> {
        let bytes_to_write = self.0.as_ref().unwrap().to_bytes();
        let sum = CRC8.checksum(bytes_to_write.as_slice());
        let len = bytes_to_write.len() as u32 + 1;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(bytes_to_write.as_slice());
        data.push(sum);
        data
    }
}

impl Decoder for CrcBuffer {
    fn decode_bytes(&mut self, data: &[u8]) -> u32 {
        let item_len = u32::from_be_bytes(
            data[0..4].try_into().unwrap()
        );
        let bytes_to_decode = &data[4..(3 + item_len as usize)];
        let sum = data[3 + item_len as usize];
        if CRC8.checksum(bytes_to_decode) == sum {
            self.0 = Some(Buffer::from_encoded_bytes(bytes_to_decode))
        }
        4 + item_len
    }
}

impl Take for CrcBuffer {
    fn take(self) -> Option<Buffer> {
        self.0
    }
}

impl PartialEq for CrcBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[cfg(test)]
mod tests {
    use crate::core::buffer::{Buffer, Decoder, Encoder};
    use crate::core::crc::CrcBuffer;

    #[test]
    fn test_crc_buffer() {
        let buffer = Buffer::from_i32("key", 1);
        let buffer = CrcBuffer::new_with_buffer(buffer);
        let bytes = buffer.encode_to_bytes();
        let mut copy = CrcBuffer::new();
        copy.decode_bytes(bytes.as_slice());
        assert_eq!(copy, buffer)
    }
}