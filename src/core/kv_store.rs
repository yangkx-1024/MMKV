use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::RwLock;
use crate::core::buffer::{Buffer, Encoder, Take};
use crate::core::crc::CrcBuffer;
use crate::core::crypt::{Crypt, CryptBuffer};
use crate::core::memory_map::MemoryMap;

const _META_FILE_NAME: &str = "mmkv_meta";

#[derive(Debug)]
pub struct KvStore {
    _kv_map: HashMap<String, Buffer>,
    _file: File,
    _meta_file_path: PathBuf,
    _mm: RwLock<MemoryMap>,
    _file_size: u64,
    _page_size: u64,
    _crypt: Option<Rc<RefCell<Crypt>>>,
}

impl KvStore {
    pub fn new(path: &Path, page_size: u64) -> Self {
        KvStore::new_kv_store(path, page_size, None)
    }

    pub fn new_with_encrypt_key(path: &Path, page_size: u64, key: &str) -> Self {
        KvStore::new_kv_store(path, page_size, Some(key))
    }

    fn new_kv_store(path: &Path, page_size: u64, key: Option<&str>) -> Self {
        let file = OpenOptions::new().read(true).write(true).create(true).open(path).unwrap();
        let mut file_len = file.metadata().unwrap().len();
        if file_len == 0 {
            file_len += page_size;
            file.set_len(file_len).unwrap();
        }
        let meta_file_path = path.parent().expect("unable to access dir").join(_META_FILE_NAME);
        let crypt = key.map(|raw_key| {
            let key = hex::decode(raw_key).unwrap();
            Rc::new(RefCell::new(
                KvStore::init_crypt(&meta_file_path, key.as_slice())
            ))
        });

        let mm = MemoryMap::new(&file);
        let mut store = KvStore {
            _kv_map: HashMap::new(),
            _file: file,
            _meta_file_path: meta_file_path,
            _mm: RwLock::new(mm),
            _file_size: file_len,
            _page_size: page_size,
            _crypt: crypt,
        };
        store.init();
        store
    }

    fn init(&mut self) {
        // acquire write lock
        let mm = self._mm.read().unwrap();
        let decode = |buffer: Option<Buffer>| {
            if let Some(data) = buffer {
                self._kv_map.insert(data.key().to_string(), data);
            }
        };
        match self._crypt.as_ref() {
            Some(crypt) => {
                mm.iter(|| CryptBuffer::new(crypt.clone())).for_each(decode)
            }
            None => {
                mm.iter(|| CrcBuffer::new()).for_each(decode)
            }
        };
    }

    fn init_crypt(meta_path: &PathBuf, key: &[u8]) -> Crypt {
        if meta_path.exists() {
            let mut nonce_file = OpenOptions::new().read(true).open(meta_path).unwrap();
            let mut nonce = Vec::<u8>::new();
            nonce_file.read_to_end(&mut nonce).expect("failed to read nonce file");
            Crypt::new_with_nonce(key.try_into().unwrap(), nonce.as_slice())
        } else {
            let crypt = Crypt::new(key.try_into().unwrap());
            let mut nonce_file = OpenOptions::new().create(true).write(true)
                .open(meta_path)
                .unwrap();
            nonce_file.write_all(crypt.nonce().as_slice()).expect("failed to write nonce file");
            nonce_file.sync_all().unwrap();
            crypt
        }
    }

    pub fn write(&mut self, key: &str, buffer: Buffer) {
        if let Some(crypt) = self._crypt.clone() {
            self.write_internal(key, buffer, |buffer: Buffer| CryptBuffer::new_with_buffer(buffer, crypt.clone()))
        } else {
            self.write_internal(key, buffer, |buffer: Buffer| CrcBuffer::new_with_buffer(buffer))
        };
    }

    fn write_internal<T, F>(&mut self, key: &str, raw_buffer: Buffer, transform: F) where T: Encoder + Take, F: Fn(Buffer) -> T {
        // acquire write lock
        let mut mm = self._mm.write().unwrap();
        let buffer = transform(raw_buffer);
        let mut data = buffer.encode_to_bytes();
        let mut target_end = data.len() + mm.len();
        if target_end as u64 > self._file_size {
            // trim the file, drop the duplicate items
            let mut crypt_changed = false;
            if let Some(crypt) = self._crypt.clone() {
                let _ = fs::remove_file(self._meta_file_path.to_path_buf());
                let key = crypt.borrow().key();
                *crypt.borrow_mut() = KvStore::init_crypt(
                    &self._meta_file_path, key.as_slice(),
                );
                crypt_changed = true;
            }
            let mut vec: Vec<u8> = vec!();
            for buffer in self._kv_map.values() {
                let buffer = transform(buffer.clone());
                vec.extend_from_slice(buffer.encode_to_bytes().as_slice());
            }
            mm.write_all(vec).unwrap();
            if crypt_changed {
                data = buffer.encode_to_bytes();
            }
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
        self._kv_map.insert(key.to_string(), buffer.take().unwrap());
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

    use crate::core::buffer::Buffer;
    use crate::core::crc::CrcBuffer;
    use crate::core::kv_store::{_META_FILE_NAME, KvStore};

    #[test]
    fn test_normal_kv_store() {
        let _ = fs::remove_file("test_kv_store");
        test_kv_store(|| KvStore::new(Path::new("test_kv_store"), 100));
        let _ = fs::remove_file("test_kv_store");
    }

    #[test]
    fn test_crypt_kv_store() {
        let _ = fs::remove_file("test_crypt_kv_store");
        let _ = fs::remove_file(_META_FILE_NAME);
        test_kv_store(|| KvStore::new_with_encrypt_key(
            Path::new("test_crypt_kv_store"),
            100,
            "88C51C536176AD8A8EE4A06F62EE897E",
        ));
        let _ = fs::remove_file("test_crypt_kv_store");
        let _ = fs::remove_file(_META_FILE_NAME);
    }

    fn test_kv_store<F>(f: F) where F: Fn() -> KvStore {
        let mut store = f();
        store.write("key1", Buffer::from_i32("key1", 1));
        assert_eq!(store.get("key1").unwrap().decode_i32().unwrap(), 1);
        assert_eq!(store.get("key2").is_none(), true);
        store.write("key2", Buffer::from_i32("key2", 2));
        assert_eq!(store.get("key2").unwrap().decode_i32().unwrap(), 2);
        store.write("key3", Buffer::from_i32("key3", 3));
        assert_eq!(store.get("key3").unwrap().decode_i32().unwrap(), 3);

        store.write("key1", Buffer::from_str("key1", "4"));
        assert_eq!(store.get("key1").unwrap().decode_i32(), None);
        assert_eq!(store.get("key1").unwrap().decode_str().unwrap(), "4");

        drop(store);

        let mut store = f();
        assert_eq!(store.get("key1").unwrap().decode_str().unwrap(), "4");
        assert_eq!(store.get("key2").unwrap().decode_i32().unwrap(), 2);
        assert_eq!(store.get("key3").unwrap().decode_i32().unwrap(), 3);

        store.write("key4", Buffer::from_i32("key4", 4));
        store.write("key5", Buffer::from_i32("key5", 5));
        store.write("key6", Buffer::from_i32("key6", 6));
        store.write("key7", Buffer::from_i32("key7", 7));
        store.write("key8", Buffer::from_i32("key8", 8));
        store.write("key9", Buffer::from_i32("key9", 9));
        assert_eq!(store.get("key9").unwrap().decode_i32().unwrap(), 9);
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
            let handle = spawn(move || {
                for i in 0..10000 {
                    STORE.get_mut().unwrap().write(key, Buffer::from_i32(key, i));
                }
            });
            for i in 0..10000 {
                STORE.get_mut().unwrap().write(key, Buffer::from_i32(key, i));
            }
            handle.join().unwrap();
            println!("value: {}", STORE.get().unwrap().get(key).unwrap().decode_i32().unwrap());
            println!("{}", STORE.get().unwrap());
            let count = STORE.get().unwrap()._mm.read().unwrap().iter(|| CrcBuffer::new()).count();
            assert_eq!(count, 20000)
        }
        let _ = fs::remove_file("test_kv_store_multi_thread");
    }
}