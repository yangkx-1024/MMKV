use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::RwLock;
use crate::core::buffer::Buffer;
use crate::core::memory_map::MemoryMap;

#[derive(Debug)]
pub struct KVStore {
    _kv_map: HashMap<String, Buffer>,
    _file: File,
    _mm: RwLock<MemoryMap>,
    _page_size: u64,
}

pub trait ContentContainer {
    fn max_len(&self) -> usize;
    fn content_len(&self) -> usize;
    fn append(&mut self, value: Vec<u8>) -> std::io::Result<()>;
    fn write_all(&mut self, value: Vec<u8>) -> std::io::Result<()>;
    fn read(&self, offset: usize) -> &[u8];
}

impl <'a> KVStore {
    pub fn new(path: &Path, page_size: u64) -> Self {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        let file_len = file.metadata().unwrap().len();
        if file_len == 0 {
            file.set_len(page_size).unwrap();
        }
        let mm = MemoryMap::new(&file);
        let mut store = KVStore {
            _kv_map: HashMap::new(),
            _file: file,
            _mm: RwLock::new(mm),
            _page_size: page_size,
        };
        store.init();
        store
    }

    fn init(&mut self) {
        // acquire write lock
        let mm = self._mm.read().unwrap();
        let len = mm.content_len();
        let mut read_len = 0;
        while read_len < len {
            let buffer = Buffer::from_encoded_bytes(mm.read(read_len));
            read_len = read_len + buffer.len() as usize;
            self._kv_map.insert(buffer.key().to_string(), buffer);
        }
    }

    pub fn write(&mut self, key: &str, buffer: Buffer) {
        // acquire write lock
        let mm = self._mm.get_mut().unwrap();
        let data = buffer.to_bytes();
        let mut target_end = data.len() + mm.content_len();
        if target_end > mm.max_len() {
            // trim the file, drop the duplicate items
            let mut vec: Vec<u8> = vec!();
            for (_, buffer) in self._kv_map.iter() {
                vec.extend_from_slice(buffer.to_bytes().as_slice());
            }
            mm.write_all(vec).unwrap();
            target_end = data.len() + mm.content_len()
        }
        while target_end > mm.max_len() {
            // expand the file size with _page_size
            self._file.sync_all().unwrap();
            let len = self._file.metadata().unwrap().len() + self._page_size;
            self._file.set_len(len).unwrap();
            *mm = MemoryMap::new(&self._file);
        }
        mm.append(data).unwrap();
        self._kv_map.insert(key.to_string(), buffer);
    }

    pub fn get(&self, key: &str) -> Option<&Buffer> {
        // acquire read lock
        let _mm = self._mm.read();
        self._kv_map.get(key)
    }

    pub fn dump(&self) {
        println!("file size: {}", self._file.metadata().unwrap().len());
        println!("key count: {}", self._kv_map.len());
        println!("content len: {}", self._mm.read().unwrap().content_len());
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use crate::core::kv_store::{ContentContainer, KVStore};
    use crate::core::buffer::Buffer;

    #[test]
    fn test_kv_store() {
        let _ = fs::remove_file("kv_store_test.txt");
        let mut store = KVStore::new(Path::new("kv_store_test.txt"), 100);
        store.write("key1", Buffer::from_kv("key1", 1u8.to_be_bytes().as_slice()));
        assert_eq!(store.get("key1").unwrap().value()[0], 1);
        assert_eq!(store.get("key2").is_none(), true);
        store.write("key2", Buffer::from_kv("key2", 2u8.to_be_bytes().as_slice()));
        assert_eq!(store.get("key2").unwrap().value()[0], 2);
        store.write("key3", Buffer::from_kv("key3", 3u8.to_be_bytes().as_slice()));
        assert_eq!(store.get("key3").unwrap().value()[0], 3);
        assert_eq!(store._mm.read().unwrap().content_len(), 39);

        store.write("key1", Buffer::from_kv("key1", 4u8.to_be_bytes().as_slice()));
        store.write("key2", Buffer::from_kv("key2", 5u8.to_be_bytes().as_slice()));
        store.write("key3", Buffer::from_kv("key3", 6u8.to_be_bytes().as_slice()));
        assert_eq!(store.get("key1").unwrap().value()[0], 4);
        assert_eq!(store.get("key2").unwrap().value()[0], 5);
        assert_eq!(store.get("key3").unwrap().value()[0], 6);
        assert_eq!(store._mm.read().unwrap().content_len(), 78);
        store.write("key1", Buffer::from_kv("key1", 1u8.to_be_bytes().as_slice()));
        assert_eq!(store._mm.read().unwrap().content_len(), 91);
        assert_eq!(store.get("key1").unwrap().value()[0], 1);
        store.write("key2", Buffer::from_kv("key2", 2u8.to_be_bytes().as_slice()));
        assert_eq!(store._mm.read().unwrap().content_len(), 52);
        assert_eq!(store.get("key2").unwrap().value()[0], 2);
        assert_eq!(store.get("key3").unwrap().value()[0], 6);

        drop(store);
        let mut store = KVStore::new(Path::new("kv_store_test.txt"), 100);
        assert_eq!(store.get("key1").unwrap().value()[0], 1);
        assert_eq!(store.get("key2").unwrap().value()[0], 2);
        assert_eq!(store.get("key3").unwrap().value()[0], 6);
        assert_eq!(store._mm.read().unwrap().content_len(), 52);

        store.write("key4", Buffer::from_kv("key4", 4u8.to_be_bytes().as_slice()));
        store.write("key5", Buffer::from_kv("key4", 5u8.to_be_bytes().as_slice()));
        store.write("key6", Buffer::from_kv("key4", 6u8.to_be_bytes().as_slice()));
        store.write("key7", Buffer::from_kv("key4", 7u8.to_be_bytes().as_slice()));
        store.write("key8", Buffer::from_kv("key4", 8u8.to_be_bytes().as_slice()));
        store.write("key9", Buffer::from_kv("key4", 9u8.to_be_bytes().as_slice()));
        assert_eq!(store._file.metadata().unwrap().len(), 200);
        assert_eq!(store._mm.read().unwrap().max_len(), 192);
        assert_eq!(store.get("key9").unwrap().value()[0], 9);
        let _ = fs::remove_file("kv_store_test.txt");
    }
}