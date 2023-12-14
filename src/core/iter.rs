use crate::core::buffer::{Buffer, DecodeResult};
use crate::core::memory_map::{MemoryMap, LEN_OFFSET};

const LOG_TAG: &str = "MMKV:MemoryMap";

pub struct Iter<'a, F>
where
    F: Fn(&[u8]) -> crate::Result<DecodeResult>,
{
    mm: &'a MemoryMap,
    start: usize,
    end: usize,
    decode: F,
}

impl MemoryMap {
    pub fn iter<F>(&self, decode: F) -> Iter<F>
    where
        F: Fn(&[u8]) -> crate::Result<DecodeResult>,
    {
        let start = LEN_OFFSET;
        let end = self.len();
        Iter {
            mm: self,
            start,
            end,
            decode,
        }
    }
}

impl<'a, F> Iterator for Iter<'a, F>
where
    F: Fn(&[u8]) -> crate::Result<DecodeResult>,
{
    type Item = Option<Buffer>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }
        let bytes = self.mm.read(self.start..self.end);
        let decode_result = (self.decode)(bytes);
        match decode_result {
            Ok(result) => {
                self.start += result.len as usize;
                Some(result.buffer)
            }
            Err(e) => {
                error!(LOG_TAG, "Failed to iter memory map, reason: {:?}", e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::OpenOptions;
    use std::mem::size_of;

    use crate::core::buffer::{Buffer, DecodeResult, Decoder, Encoder};
    use crate::core::memory_map::MemoryMap;
    use crate::Error::DataInvalid;
    use crate::Result;

    const LOG_TAG: &str = "MMKV:IterTest";

    struct TestEncoderDecoder;
    impl Encoder for TestEncoderDecoder {
        fn encode_to_bytes(&self, raw_buffer: &Buffer) -> Result<Vec<u8>> {
            let bytes_to_write = raw_buffer.to_bytes();
            let len = bytes_to_write.len() as u32;
            let mut data = len.to_be_bytes().to_vec();
            data.extend_from_slice(bytes_to_write.as_slice());
            Ok(data)
        }
    }

    impl Decoder for TestEncoderDecoder {
        fn decode_bytes(&self, data: &[u8]) -> Result<DecodeResult> {
            let offset = size_of::<u32>();
            let item_len = u32::from_be_bytes(data[0..offset].try_into().map_err(|_| DataInvalid)?);
            let bytes_to_decode = &data[offset..(offset + item_len as usize)];
            let read_len = offset as u32 + item_len;
            let result = Buffer::from_encoded_bytes(bytes_to_decode);
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

    #[test]
    fn test_mmap_iterator() {
        let file_name = "test_mmap_iterator";
        let _ = fs::remove_file(file_name);
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(file_name)
            .unwrap();
        file.set_len(1024).unwrap();
        let mut mm = MemoryMap::new(&file);
        let mut buffers: Vec<Buffer> = vec![];
        let test_encoder = &TestEncoderDecoder;
        for i in 0..10 {
            let buffer = Buffer::from_i32(&i.to_string(), i);
            mm.append(test_encoder.encode_to_bytes(&buffer).unwrap())
                .unwrap();
            buffers.push(buffer);
        }
        let decoder = &TestEncoderDecoder;
        for (index, i) in mm.iter(|bytes| decoder.decode_bytes(bytes)).enumerate() {
            assert_eq!(buffers[index], i.unwrap());
        }
        let _ = fs::remove_file("test_mmap_iterator");
    }
}
