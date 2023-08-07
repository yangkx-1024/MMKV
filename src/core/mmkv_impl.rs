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

const META_FILE_NAME: &str = "mmkv_meta";

#[derive(Debug)]
pub struct MmkvImpl {
    kv_map: HashMap<String, Buffer>,
    file: File,
    meta_file_path: PathBuf,
    mm: RwLock<MemoryMap>,
    file_size: u64,
    page_size: u64,
    crypt: Option<Rc<RefCell<Crypt>>>,
}

impl MmkvImpl {
    pub fn new(path: &Path, page_size: u64) -> Self {
        MmkvImpl::new_mmkv(path, page_size, None)
    }

    pub fn new_with_encrypt_key(path: &Path, page_size: u64, key: &str) -> Self {
        MmkvImpl::new_mmkv(path, page_size, Some(key))
    }

    fn new_mmkv(path: &Path, page_size: u64, key: Option<&str>) -> Self {
        let file = OpenOptions::new().read(true).write(true).create(true).open(path).unwrap();
        let mut file_len = file.metadata().unwrap().len();
        if file_len == 0 {
            file_len += page_size;
            file.set_len(file_len).unwrap();
        }
        let meta_file_path = path.parent().expect("unable to access dir").join(META_FILE_NAME);
        let crypt = key.map(|raw_key| {
            let key = hex::decode(raw_key).unwrap();
            Rc::new(RefCell::new(
                MmkvImpl::init_crypt(&meta_file_path, key.as_slice())
            ))
        });

        let mm = MemoryMap::new(&file);
        let mut mmkv = MmkvImpl {
            kv_map: HashMap::new(),
            file,
            meta_file_path,
            mm: RwLock::new(mm),
            file_size: file_len,
            page_size,
            crypt,
        };
        mmkv.init();
        mmkv
    }

    fn init(&mut self) {
        // acquire write lock
        let mm = self.mm.read().unwrap();
        let decode = |buffer: Option<Buffer>| {
            if let Some(data) = buffer {
                self.kv_map.insert(data.key().to_string(), data);
            }
        };
        match self.crypt.as_ref() {
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
        if let Some(crypt) = self.crypt.clone() {
            self.write_internal(key, buffer, |buffer: Buffer| CryptBuffer::new_with_buffer(buffer, crypt.clone()))
        } else {
            self.write_internal(key, buffer, |buffer: Buffer| CrcBuffer::new_with_buffer(buffer))
        };
    }

    fn write_internal<T, F>(&mut self, key: &str, raw_buffer: Buffer, transform: F) where T: Encoder + Take, F: Fn(Buffer) -> T {
        // acquire write lock
        let mut mm = self.mm.write().unwrap();
        let buffer = transform(raw_buffer);
        let mut data = buffer.encode_to_bytes();
        let mut target_end = data.len() + mm.len();
        if target_end as u64 > self.file_size {
            // trim the file, drop the duplicate items
            let mut crypt_changed = false;
            if let Some(crypt) = self.crypt.clone() {
                let _ = fs::remove_file(self.meta_file_path.to_path_buf());
                let key = crypt.borrow().key();
                // The crypt is using stream encryption,
                // subsequent data encryption depends on the results of previous data encryption,
                // we want rewrite the entire stream, so crypt needs to be reset.
                *crypt.borrow_mut() = MmkvImpl::init_crypt(
                    &self.meta_file_path, key.as_slice(),
                );
                crypt_changed = true;
            }
            let mut vec: Vec<u8> = vec!();
            for buffer in self.kv_map.values() {
                let buffer = transform(buffer.clone());
                vec.extend_from_slice(buffer.encode_to_bytes().as_slice());
            }
            // rewrite the entire map
            mm.write_all(vec).unwrap();
            // the crypt has been reset, need encode it with new crypt
            if crypt_changed {
                data = buffer.encode_to_bytes();
            }
            target_end = data.len() + mm.len()
        }
        while target_end as u64 > self.file_size {
            // expand the file size with page_size
            self.file.sync_all().unwrap();
            self.file_size += self.page_size;
            self.file.set_len(self.file_size).unwrap();
            *mm = MemoryMap::new(&self.file);
        }
        mm.append(data).unwrap();
        self.kv_map.insert(key.to_string(), buffer.take().unwrap());
    }

    pub fn get(&self, key: &str) -> Option<&Buffer> {
        // acquire read lock
        let _mm = self.mm.read();
        self.kv_map.get(key)
    }
}

impl Display for MmkvImpl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "MMKV {{ file_size: {}, key_count: {}, content_len: {} }}",
            self.file_size,
            self.kv_map.len(),
            self.mm.read().unwrap().len()
        )
    }
}

impl Drop for MmkvImpl {
    fn drop(&mut self) {
        self.file.sync_all().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, thread};
    use std::path::Path;
    use std::sync::OnceLock;
    use std::thread::{JoinHandle, spawn};

    use crate::core::buffer::Buffer;
    use crate::core::crc::CrcBuffer;
    use crate::core::mmkv_impl::{META_FILE_NAME, MmkvImpl};

    #[test]
    fn test_normal_mmkv() {
        let _ = fs::remove_file("test_normal_mmkv");
        test_mmkv(|| MmkvImpl::new(Path::new("test_normal_mmkv"), 100));
        let _ = fs::remove_file("test_normal_mmkv");
    }

    #[test]
    fn test_crypt_mmkv() {
        let _ = fs::remove_file("test_crypt_mmkv");
        let _ = fs::remove_file(META_FILE_NAME);
        test_mmkv(|| MmkvImpl::new_with_encrypt_key(
            Path::new("test_crypt_mmkv"),
            100,
            "88C51C536176AD8A8EE4A06F62EE897E",
        ));
        let _ = fs::remove_file("test_crypt_mmkv");
        let _ = fs::remove_file(META_FILE_NAME);
    }

    fn test_mmkv<F>(f: F) where F: Fn() -> MmkvImpl {
        let mut mmkv = f();
        mmkv.write("key1", Buffer::from_i32("key1", 1));
        assert_eq!(mmkv.get("key1").unwrap().decode_i32().unwrap(), 1);
        assert_eq!(mmkv.get("key2").is_none(), true);
        mmkv.write("key2", Buffer::from_i32("key2", 2));
        assert_eq!(mmkv.get("key2").unwrap().decode_i32().unwrap(), 2);
        mmkv.write("key3", Buffer::from_i32("key3", 3));
        assert_eq!(mmkv.get("key3").unwrap().decode_i32().unwrap(), 3);

        mmkv.write("key1", Buffer::from_str("key1", "4"));
        assert_eq!(mmkv.get("key1").unwrap().decode_i32(), None);
        assert_eq!(mmkv.get("key1").unwrap().decode_str().unwrap(), "4");

        drop(mmkv);

        let mut mmkv = f();
        assert_eq!(mmkv.get("key1").unwrap().decode_str().unwrap(), "4");
        assert_eq!(mmkv.get("key2").unwrap().decode_i32().unwrap(), 2);
        assert_eq!(mmkv.get("key3").unwrap().decode_i32().unwrap(), 3);

        mmkv.write("key4", Buffer::from_i32("key4", 4));
        mmkv.write("key5", Buffer::from_i32("key5", 5));
        mmkv.write("key6", Buffer::from_i32("key6", 6));
        mmkv.write("key7", Buffer::from_i32("key7", 7));
        mmkv.write("key8", Buffer::from_i32("key8", 8));
        mmkv.write("key9", Buffer::from_i32("key9", 9));
        assert_eq!(mmkv.get("key9").unwrap().decode_i32().unwrap(), 9);
    }

    #[test]
    fn test_multi_thread_mmkv() {
        let _ = fs::remove_file("test_multi_thread_mmkv");
        static mut MMKV: OnceLock<MmkvImpl> = OnceLock::new();
        unsafe {
            MMKV.set(
                MmkvImpl::new(Path::new("test_multi_thread_mmkv"), 4096)
            ).unwrap();
            let action = || {
                let current_thread = thread::current();
                let thread_id = current_thread.id();
                for i in 0..5000 {
                    let key = &format!("{:?}_key_{i}", thread_id);
                    MMKV.get_mut().unwrap().write(key, Buffer::from_i32(key, i));
                }
            };
            let mut threads = Vec::<JoinHandle<()>>::new();
            for _ in 0..4 {
                threads.push(spawn(action));
            }
            for handle in threads {
                handle.join().unwrap()
            }
            let count = MMKV.get().unwrap().mm.read().unwrap().iter(|| CrcBuffer::new()).count();
            assert_eq!(count, 20000)
        }
        let _ = fs::remove_file("test_multi_thread_mmkv");
    }
}