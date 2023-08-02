use std::fs::File;
use memmap2::MmapMut;
use crate::core::kv_store::ContentContainer;

const _LEN_OFFSET: usize = 8;

#[derive(Debug)]
pub struct MemoryMap {
    inner_map: MmapMut,
}

impl MemoryMap {
    pub fn new(file: &File) -> Self {
        MemoryMap {
            inner_map: unsafe { MmapMut::map_mut(file) }.unwrap(),
        }
    }
}

impl ContentContainer for MemoryMap {
    fn max_len(&self) -> usize {
        self.inner_map.len() - _LEN_OFFSET
    }

    fn content_len(&self) -> usize {
        usize::from_be_bytes(
            self.inner_map[0.._LEN_OFFSET].try_into().unwrap()
        )
    }

    fn append(&mut self, value: Vec<u8>) -> std::io::Result<()> {
        let data_len = value.len();
        let content_len = self.content_len();
        let start = content_len + _LEN_OFFSET;
        let end = data_len + start;
        let new_content_len = data_len + content_len;
        self.inner_map[0.._LEN_OFFSET].copy_from_slice(new_content_len.to_be_bytes().as_slice());
        self.inner_map[start..end].copy_from_slice(value.as_slice());
        self.inner_map.flush()
    }

    fn write_all(&mut self, value: Vec<u8>) -> std::io::Result<()> {
        let data_len = value.len();
        let start = _LEN_OFFSET;
        let end = start + data_len;
        self.inner_map[0.._LEN_OFFSET].copy_from_slice(data_len.to_be_bytes().as_slice());
        self.inner_map[start..end].copy_from_slice(value.as_slice());
        self.inner_map.flush()
    }

    fn read(&self, offset: usize) -> &[u8] {
        self.inner_map[(offset + _LEN_OFFSET)..].as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::OpenOptions;
    use crate::core::kv_store::ContentContainer;
    use super::MemoryMap;

    #[test]
    fn test_mmap() {
        let _ = fs::remove_file("memory_map_test.txt");
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open("memory_map_test.txt")
            .unwrap();
        file.set_len(1024).unwrap();
        let mut mm = MemoryMap::new(&file);
        assert_eq!(mm.max_len(), 1016);
        assert_eq!(mm.content_len(), 0);
        mm.append(vec![1u8, 2u8, 3u8]).unwrap();
        assert_eq!(mm.content_len(), 3);
        mm.append(vec![4u8]).unwrap();
        assert_eq!(mm.content_len(), 4);

        let read = mm.read(0);
        assert_eq!(read[0], 1u8);
        assert_eq!(read[1], 2u8);
        let read = mm.read(mm.content_len() - 1);
        assert_eq!(read[0], 4u8);

        mm.write_all(vec![5, 4, 3, 2, 1]).unwrap();
        assert_eq!(mm.content_len(), 5);
        let read = mm.read(0);
        assert_eq!(read[0], 5);

        let read = mm.read(1);
        assert_eq!(read[0], 4);
        let _ = fs::remove_file("memory_map_test.txt");
    }
}