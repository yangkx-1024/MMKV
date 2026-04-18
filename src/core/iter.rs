use crate::core::buffer::{Buffer, DecodeResult, SliceLoc};
use crate::core::memory_map::MemoryMap;
use std::collections::HashMap;

const LOG_TAG: &str = "MMKV:MemoryMap";

pub struct Iter<'a, F>
where
    F: Fn(&[u8], u32) -> crate::Result<DecodeResult>,
{
    mm: &'a MemoryMap,
    pub position: u32,
    start: usize,
    end: usize,
    decode: F,
}

impl MemoryMap {
    pub fn iter<F>(&self, decode: F) -> crate::Result<Iter<'_, F>>
    where
        F: Fn(&[u8], u32) -> crate::Result<DecodeResult>,
    {
        let start = self.content_start_offset();
        let end = self.write_offset()?;
        Ok(Iter {
            mm: self,
            position: 0,
            start,
            end,
            decode,
        })
    }
}

impl<F> Iter<'_, F>
where
    F: Fn(&[u8], u32) -> crate::Result<DecodeResult>,
{
    pub fn into_map(mut self, mmap_base: usize) -> (HashMap<String, Buffer>, u32) {
        let mut iter_count = 0u32;
        let mut map = HashMap::new();

        loop {
            let record_start = self.start;
            if self.start >= self.end {
                break;
            }
            let bytes = match self.mm.read(self.start..self.end) {
                Ok(b) => b,
                Err(e) => {
                    error!(LOG_TAG, "Failed to read memory map: {:?}", e);
                    break;
                }
            };
            let position = self.position;
            let decode_result = match (self.decode)(bytes, position) {
                Ok(r) => r,
                Err(e) => {
                    error!(LOG_TAG, "Failed to iter memory map: {:?}", e);
                    break;
                }
            };
            self.position += 1;
            iter_count += 1;
            let record_len = decode_result.len as usize;
            self.start += record_len;

            let buffer = match decode_result.buffer {
                Some(b) => b,
                None => continue,
            };

            if buffer.is_deleting() {
                // Tombstone: remove key. For Owned we have the key; Slice shouldn't appear here.
                if let Buffer::Owned { ref kv, .. } = buffer {
                    map.remove(kv.key.as_str());
                }
                continue;
            }

            // Build a Slice entry pointing into the mmap.
            let (key, type_token) = match &buffer {
                Buffer::Owned { kv, .. } => (kv.key.clone(), kv.r#type),
                Buffer::Slice(_) => continue, // shouldn't happen from decode
            };

            let slice_buf =
                SliceLoc::from_record(mmap_base, record_start, record_len, type_token, position)
                    .map(Buffer::Slice);
            map.insert(key, slice_buf.unwrap_or(buffer));
        }

        (map, iter_count)
    }
}

impl<F> Iterator for Iter<'_, F>
where
    F: Fn(&[u8], u32) -> crate::Result<DecodeResult>,
{
    type Item = Option<Buffer>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }
        let bytes = match self.mm.read(self.start..self.end) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!(LOG_TAG, "Failed to read memory map, reason: {:?}", e);
                return None;
            }
        };
        let decode_result = (self.decode)(bytes, self.position);
        self.position += 1;
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

    use crate::Error::DataInvalid;
    use crate::Result;
    use crate::core::buffer::{Buffer, DecodeResult, Decoder, Encoder, encode_kv_bytes};
    use crate::core::memory_map::MemoryMap;

    const LOG_TAG: &str = "MMKV:IterTest";

    struct TestEncoderDecoder;

    impl Encoder for TestEncoderDecoder {
        fn encode_to_bytes(
            &self,
            key: &str,
            type_token: i32,
            value: &[u8],
            _position: u32,
        ) -> Result<Vec<u8>> {
            let bytes_to_write = encode_kv_bytes(key, type_token, value);
            let len = bytes_to_write.len() as u32;
            let mut data = len.to_be_bytes().to_vec();
            data.extend_from_slice(bytes_to_write.as_slice());
            Ok(data)
        }
    }

    impl Decoder for TestEncoderDecoder {
        fn decode_bytes(&self, data: &[u8], _: u32) -> Result<DecodeResult> {
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
            .truncate(true)
            .write(true)
            .read(true)
            .open(file_name)
            .unwrap();
        file.set_len(1024).unwrap();
        let mut mm = MemoryMap::new(&file, 1024).unwrap();
        let mut buffers: Vec<Buffer> = vec![];
        let test_encoder = &TestEncoderDecoder;
        for i in 0..10i32 {
            let buffer = Buffer::new(&i.to_string(), i);
            mm.append(
                &test_encoder
                    .encode_to_bytes(
                        &i.to_string(),
                        buffer.kv_type(),
                        buffer.kv_value(),
                        i as u32,
                    )
                    .unwrap(),
            )
            .unwrap();
            buffers.push(buffer);
        }
        let mmap_base = mm.base_ptr();
        let decoder = &TestEncoderDecoder;
        let (map, count) = mm
            .iter(|bytes, position| decoder.decode_bytes(bytes, position))
            .unwrap()
            .into_map(mmap_base);
        assert_eq!(count, 10);
        for i in 0..10i32 {
            let key = i.to_string();
            // TestEncoderDecoder doesn't have CRC framing so Slice construction falls back to Owned
            assert!(map.contains_key(&key));
        }
        let _ = fs::remove_file("test_mmap_iterator");
    }
}
