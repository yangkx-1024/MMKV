use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::RwLock;

use crate::core::buffer::Buffer;
use crate::core::memory_map::MemoryMap;

#[derive(Debug)]
pub struct KvStore {
    _kv_map: HashMap<String, Buffer>,
    _file: File,
    _mm: RwLock<MemoryMap>,
    _file_size: u64,
    _page_size: u64,
}

impl<'a> KvStore {
    pub fn new(path: &Path, page_size: u64) -> Self {
        let file = OpenOptions::new().read(true).write(true).create(true).open(path).unwrap();
        let mut file_len = file.metadata().unwrap().len();
        if file_len == 0 {
            file_len += page_size;
            file.set_len(file_len).unwrap();
        }
        let mm = MemoryMap::new(&file);
        let mut store = KvStore {
            _kv_map: HashMap::new(),
            _file: file,
            _mm: RwLock::new(mm),
            _file_size: file_len,
            _page_size: page_size,
        };
        store.init();
        store
    }

    fn init(&mut self) {
        // acquire write lock
        let mm = self._mm.read().unwrap();
        for buffer in mm.iter() {
            self._kv_map.insert(buffer.key().to_string(), buffer);
        }
    }

    pub fn write(&mut self, key: &str, buffer: Buffer) {
        // acquire write lock
        let mut mm = self._mm.write().unwrap();
        let data = buffer.to_bytes();
        let mut target_end = data.len() + mm.len();
        if target_end as u64 > self._file_size {
            // trim the file, drop the duplicate items
            let mut vec: Vec<u8> = vec!();
            for (_, buffer) in self._kv_map.iter() {
                vec.extend_from_slice(buffer.to_bytes().as_slice());
            }
            mm.write_all(vec).unwrap();
            target_end = data.len() + mm.len()
        }
        while target_end as u64 > self._file_size {
            // expand the file size with _page_size
            self._file.sync_all().unwrap();
            self._file_size += self._page_size;
            self._file.set_len(self._file_size).unwrap();
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
}

impl Display for KvStore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "KvStore {{ file_size: {}, key_count: {}, content_len: {} }}",
            self._file_size,
            self._kv_map.len(),
            self._mm.read().unwrap().len()
        )
    }
}

impl Drop for KvStore {
    fn drop(&mut self) {
        self._file.sync_all().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::OnceLock;
    use std::thread::spawn;

    use crate::core::buffer::{Buffer, Decoder};
    use crate::core::kv_store::KvStore;

    #[test]
    fn test_kv_store() {
        let _ = fs::remove_file("test_kv_store");
        let mut store = KvStore::new(Path::new("test_kv_store"), 100);
        store.write("key1", Buffer::from_i32("key1", 1));
        assert_eq!(store._mm.read().unwrap().len(), 24);
        assert_eq!(store.get("key1").unwrap().decode_i32().unwrap(), 1);
        assert_eq!(store.get("key2").is_none(), true);
        store.write("key2", Buffer::from_i32("key2", 2));
        assert_eq!(store.get("key2").unwrap().decode_i32().unwrap(), 2);
        store.write("key3", Buffer::from_i32("key3", 3));
        assert_eq!(store.get("key3").unwrap().decode_i32().unwrap(), 3);
        assert_eq!(store._mm.read().unwrap().len(), 56);

        store.write("key1", Buffer::from_i32("key1", 4));
        store.write("key2", Buffer::from_i32("key2", 5));
        store.write("key3", Buffer::from_i32("key3", 6));
        assert_eq!(store.get("key1").unwrap().decode_i32().unwrap(), 4);
        assert_eq!(store.get("key2").unwrap().decode_i32().unwrap(), 5);
        assert_eq!(store.get("key3").unwrap().decode_i32().unwrap(), 6);
        assert_eq!(store._mm.read().unwrap().len(), 72);
        store.write("key1", Buffer::from_i32("key1", 1));
        assert_eq!(store._mm.read().unwrap().len(), 88);
        assert_eq!(store.get("key1").unwrap().decode_i32().unwrap(), 1);
        store.write("key2", Buffer::from_i32("key2", 2));
        assert_eq!(store._mm.read().unwrap().len(), 72);
        assert_eq!(store.get("key2").unwrap().decode_i32().unwrap(), 2);
        assert_eq!(store.get("key3").unwrap().decode_i32().unwrap(), 6);
        assert_eq!(store._file_size, 100);

        drop(store);

        let mut store = KvStore::new(Path::new("test_kv_store"), 100);
        assert_eq!(store.get("key1").unwrap().decode_i32().unwrap(), 1);
        assert_eq!(store.get("key2").unwrap().decode_i32().unwrap(), 2);
        assert_eq!(store.get("key3").unwrap().decode_i32().unwrap(), 6);
        assert_eq!(store._mm.read().unwrap().len(), 72);

        store.write("key4", Buffer::from_i32("key4", 4));
        store.write("key5", Buffer::from_i32("key5", 5));
        store.write("key6", Buffer::from_i32("key6", 6));
        store.write("key7", Buffer::from_i32("key7", 7));
        store.write("key8", Buffer::from_i32("key8", 8));
        store.write("key9", Buffer::from_i32("key9", 9));
        assert_eq!(store.get("key9").unwrap().decode_i32().unwrap(), 9);
        assert_eq!(store._file_size, 200);
        let _ = fs::remove_file("test_kv_store");
    }

    #[test]
    fn test_kv_store_multi_thread() {
        let _ = fs::remove_file("test_kv_store_multi_thread");
        static mut STORE: OnceLock<KvStore> = OnceLock::new();
        unsafe {
            STORE.set(
                KvStore::new(Path::new("test_kv_store_multi_thread"), 1024 * 1024)
            ).unwrap();
            let key = "key";
            STORE.get_mut().unwrap().write(key, Buffer::from_i32(key, 0));
            let len = STORE.get().unwrap()._mm.read().unwrap().len() - 8;
            let handle = spawn(move || {
                for i in 0..10000 {
                    STORE.get_mut().unwrap().write(key, Buffer::from_i32(key, i));
                }
            });
            for i in 0..10000 {
                STORE.get_mut().unwrap().write(key, Buffer::from_i32(key, i));
            }
            println!("value: {}", STORE.get().unwrap().get(key).unwrap().decode_i32().unwrap());
            println!("{}", STORE.get().unwrap());
            handle.join().unwrap();
            assert_eq!(STORE.get().unwrap()._mm.read().unwrap().len() - 8, len + len * 20000)
        }
        let _ = fs::remove_file("test_kv_store_multi_thread");
    }
}