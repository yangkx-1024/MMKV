use crate::core::buffer::{Buffer, Decoder, Take};
use crate::core::memory_map::{_LEN_OFFSET, MemoryMap};

pub struct Iter<'a, T: Sized, F> where T: Decoder + Take, F: Fn() -> T {
    mm: &'a MemoryMap,
    start: usize,
    end: usize,
    buffer_allocator: F,
}

impl MemoryMap {
    pub fn iter<T, F>(&self, buffer_allocator: F) -> Iter<T, F>
        where T: Decoder + Take, F: Fn() -> T {
        let start = _LEN_OFFSET;
        let end = self.len();
        Iter { mm: self, start, end, buffer_allocator }
    }
}

impl<'a, T, F> Iterator for Iter<'a, T, F> where T: Decoder + Take, F: Fn() -> T {
    type Item = Option<Buffer>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }
        let mut buffer = (self.buffer_allocator)();
        let len = buffer.decode_bytes(
            self.mm.read(self.start..self.end).as_ref()
        );
        self.start += len as usize;
        Some(buffer.take())
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
        let file = OpenOptions::new().create(true).write(true).read(true).open(file_name).unwrap();
        file.set_len(1024).unwrap();
        let mut mm = MemoryMap::new(&file);
        let mut buffers: Vec<Buffer> = vec![];
        for i in 0..10 {
            let buffer = CrcBuffer::new_with_buffer(Buffer::from_i32(&i.to_string(), i));
            mm.append(buffer.encode_to_bytes()).unwrap();
            buffers.push(buffer.take().unwrap());
        }
        let mut index = 0;
        for i in mm.iter(|| CrcBuffer::new()) {
            assert_eq!(buffers[index], i.unwrap());
            index += 1;
        }
        let _ = fs::remove_file("test_mmap_iterator");
    }
}