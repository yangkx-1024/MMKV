use crate::core::buffer::{Buffer, Decoder, Take};
use crate::core::memory_map::{MemoryMap, LEN_OFFSET};

const LOG_TAG: &str = "MMKV:MemoryMap";

pub struct Iter<'a, T: Sized, F>
where
    T: Decoder + Take,
    F: Fn() -> T,
{
    mm: &'a MemoryMap,
    start: usize,
    end: usize,
    buffer_allocator: F,
}

impl MemoryMap {
    pub fn iter<T, F>(&self, buffer_allocator: F) -> Iter<T, F>
    where
        T: Decoder + Take,
        F: Fn() -> T,
    {
        let start = LEN_OFFSET;
        let end = self.len();
        Iter {
            mm: self,
            start,
            end,
            buffer_allocator,
        }
    }
}

impl<'a, T, F> Iterator for Iter<'a, T, F>
where
    T: Decoder + Take,
    F: Fn() -> T,
{
    type Item = Option<Buffer>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }
        let mut buffer = (self.buffer_allocator)();
        let len = buffer.decode_bytes_into(self.mm.read(self.start..self.end).as_ref());
        match len {
            Ok(len) => {
                self.start += len as usize;
                Some(buffer.take())
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

    use crate::core::buffer::{Buffer, Encoder, Take};
    use crate::core::crc::CrcBuffer;
    use crate::core::memory_map::MemoryMap;

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
        for i in 0..10 {
            let buffer = CrcBuffer::new_with_buffer(Buffer::from_i32(&i.to_string(), i));
            mm.append(buffer.encode_to_bytes().unwrap()).unwrap();
            buffers.push(buffer.take().unwrap());
        }
        for (index, i) in mm.iter(CrcBuffer::new).enumerate() {
            assert_eq!(buffers[index], i.unwrap());
        }
        let _ = fs::remove_file("test_mmap_iterator");
    }
}
