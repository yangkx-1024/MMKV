use std::fs::File;
use std::ops::Range;

use memmap2::{Advice, MmapMut};

pub const _LEN_OFFSET: usize = 8;

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

    pub fn read(&self, range: Range<usize>) -> &[u8] {
        self.0[range].as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::OpenOptions;

    use super::MemoryMap;

    #[test]
    fn test_mmap() {
        let _ = fs::remove_file("test_mmap");
        let file = OpenOptions::new().create(true).write(true).read(true).open("test_mmap").unwrap();
        file.set_len(1024).unwrap();
        let mut mm = MemoryMap::new(&file);
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
}