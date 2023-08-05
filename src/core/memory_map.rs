use std::fs::File;
use std::ops::Range;

use memmap2::{Advice, MmapMut};
use crate::core::buffer::{Buffer, BufferResult};

const _LEN_OFFSET: usize = 8;
const _CRC_OFFSET: usize = 4;

#[derive(Debug)]
pub struct MemoryMap(MmapMut);

impl MemoryMap {
    pub fn new(file: &File) -> Self {
        let raw_mmap = unsafe { MmapMut::map_mut(file) }.unwrap();
        raw_mmap.advise(Advice::WillNeed).unwrap();
        MemoryMap(raw_mmap)
    }

    pub fn append(&mut self, value: Vec<u8>) -> std::io::Result<()> {
        let data_len = value.len();
        let start = self.len();
        let content_len = start - _LEN_OFFSET;
        let end = data_len + start;
        let new_content_len = data_len + content_len;
        self.0[0.._LEN_OFFSET].copy_from_slice(new_content_len.to_be_bytes().as_slice());
        self.0[start..end].copy_from_slice(value.as_slice());
        self.0.flush()
    }

    pub fn write_all(&mut self, value: Vec<u8>) -> std::io::Result<()> {
        let data_len = value.len();
        let start = _LEN_OFFSET;
        let end = start + data_len;
        self.0[0.._LEN_OFFSET].copy_from_slice(data_len.to_be_bytes().as_slice());
        self.0[start..end].copy_from_slice(value.as_slice());
        self.0.flush()
    }

    pub fn len(&self) -> usize {
        usize::from_be_bytes(
            self.0[0.._LEN_OFFSET].try_into().unwrap()
        ) + _LEN_OFFSET
    }

    fn read(&self, range: Range<usize>) -> &[u8] {
        self.0[range].as_ref()
    }
}

pub struct Iter<'a> {
    mm: &'a MemoryMap,
    start: usize,
    end: usize,
}

impl MemoryMap {
    pub fn iter(&self) -> Iter {
        let start = _LEN_OFFSET;
        let end = self.len();
        Iter {
            mm: self,
            start,
            end
        }
    }
}

impl <'a> Iterator for Iter<'a> {
    type Item = BufferResult;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None
        }
        let (buffer, len) = Buffer::from_encoded_bytes(
            self.mm.read(self.start..self.end).as_ref()
        );
        self.start += len as usize;
        Some(buffer)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::OpenOptions;
    use crate::core::buffer::Buffer;

    use super::MemoryMap;

    #[test]
    fn test_mmap() {
        let mut mm = new_mm("test_mmap");
        assert_eq!(mm.len(), 8);
        mm.append(vec![1, 2, 3]).unwrap();
        mm.append(vec![4]).unwrap();
        assert_eq!(mm.len(), 12);

        let read = mm.read(8..10);
        assert_eq!(read.len(), 2);
        assert_eq!(read[0], 1);
        assert_eq!(read[1], 2);
        let read = mm.read(mm.len() - 1..mm.len());
        assert_eq!(read[0], 4);

        mm.write_all(vec![5, 4, 3, 2, 1]).unwrap();
        assert_eq!(mm.len(), 13);
        let read = mm.read(8..9);
        assert_eq!(read[0], 5);

        let read = mm.read(9..10);
        assert_eq!(read[0], 4);
        let _ = fs::remove_file("test_mmap");
    }

    #[test]
    fn test_mmap_iterator() {
        let mut mm = new_mm("test_mmap_iterator");
        let mut buffers: Vec<Buffer> = vec![];
        for i in 0..10 {
            let buffer = Buffer::from_i32(&i.to_string(), i);
            mm.append(buffer.to_bytes()).unwrap();
            buffers.push(buffer);
        }
        let mut index = 0;
        for i in mm.iter() {
            assert_eq!(buffers[index], i.unwrap());
            index += 1;
        }
        let _ = fs::remove_file("test_mmap_iterator");
    }

    fn new_mm(file_name: &str) -> MemoryMap {
        let _ = fs::remove_file(file_name);
        let file = OpenOptions::new().create(true).write(true).read(true).open(file_name).unwrap();
        file.set_len(1024).unwrap();
        MemoryMap::new(&file)
    }
}