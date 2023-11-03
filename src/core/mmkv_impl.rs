use crate::core::buffer::{Buffer, Encoder, Take};
use crate::core::crc::CrcBuffer;
#[cfg(feature = "encryption")]
use crate::core::encrypt::{Encrypt, EncryptBuffer};
use crate::core::memory_map::MemoryMap;
use crate::Error::{EncodeFailed, InstanceClosed};
use crate::{Error, Result};
#[cfg(feature = "encryption")]
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs;
use std::fs::{File, OpenOptions};
#[cfg(feature = "encryption")]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
#[cfg(feature = "encryption")]
use std::rc::Rc;
use std::sync::RwLock;

const LOG_TAG: &str = "MMKV:Core";

#[derive(Debug)]
pub struct MmkvImpl {
    kv_map: HashMap<String, Buffer>,
    mm: RwLock<MemoryMap>,
    file_size: u64,
    page_size: u64,
    path: PathBuf,
    file: File,
    is_valid: bool,
    #[cfg(feature = "encryption")]
    meta_file_path: PathBuf,
    #[cfg(feature = "encryption")]
    encrypt: Rc<RefCell<Encrypt>>,
}

impl MmkvImpl {
    pub fn new(path: &Path, page_size: u64, #[cfg(feature = "encryption")] key: &str) -> Self {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        let mut file_len = file.metadata().unwrap().len();
        if file_len == 0 {
            file_len += page_size;
            file.set_len(file_len).unwrap();
        }
        #[cfg(feature = "encryption")]
        let meta_file_path = {
            let mut meta_ext = "meta".to_string();
            if let Some(ext) = path.extension() {
                let ext = ext.to_str().unwrap();
                meta_ext = format!("{}.meta", ext);
            }
            path.with_extension(meta_ext)
        };
        #[cfg(feature = "encryption")]
        let encrypt = {
            let key = hex::decode(key).unwrap();
            Rc::new(RefCell::new(MmkvImpl::init_encrypt(
                &meta_file_path,
                key.as_slice(),
            )))
        };
        let mm = MemoryMap::new(&file);
        let mut mmkv = MmkvImpl {
            kv_map: HashMap::new(),
            mm: RwLock::new(mm),
            file_size: file_len,
            page_size,
            path: path.to_path_buf(),
            file,
            is_valid: true,
            #[cfg(feature = "encryption")]
            meta_file_path,
            #[cfg(feature = "encryption")]
            encrypt,
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
        if cfg!(feature = "encryption") {
            #[cfg(feature = "encryption")]
            mm.iter(|| EncryptBuffer::new(self.encrypt.clone()))
                .for_each(decode)
        } else {
            mm.iter(|| CrcBuffer::new()).for_each(decode)
        }
    }

    #[cfg(feature = "encryption")]
    fn init_encrypt(meta_path: &PathBuf, key: &[u8]) -> Encrypt {
        if meta_path.exists() {
            let mut nonce_file = OpenOptions::new().read(true).open(meta_path).unwrap();
            let mut nonce = Vec::<u8>::new();
            nonce_file
                .read_to_end(&mut nonce)
                .expect("failed to read nonce file");
            Encrypt::new_with_nonce(key.try_into().unwrap(), nonce.as_slice())
        } else {
            let crypt = Encrypt::new(key.try_into().unwrap());
            let mut nonce_file = OpenOptions::new()
                .create(true)
                .write(true)
                .open(meta_path)
                .unwrap();
            nonce_file
                .write_all(crypt.nonce().as_slice())
                .expect("failed to write nonce file");
            crypt
        }
    }

    #[cfg(feature = "encryption")]
    pub fn put(&mut self, key: &str, buffer: Buffer) -> Result<()> {
        let encrypt = self.encrypt.clone();
        self.put_internal(key, buffer, |buffer: Buffer| {
            EncryptBuffer::new_with_buffer(buffer, encrypt.clone())
        })
    }

    #[cfg(not(feature = "encryption"))]
    pub fn put(&mut self, key: &str, buffer: Buffer) -> Result<()> {
        self.put_internal(key, buffer, |buffer: Buffer| {
            CrcBuffer::new_with_buffer(buffer)
        })
    }

    fn put_internal<T, F>(&mut self, key: &str, raw_buffer: Buffer, transform: F) -> Result<()>
    where
        T: Encoder + Take,
        F: Fn(Buffer) -> T,
    {
        // acquire write lock
        let mut mm = self.mm.write().map_err(|e| EncodeFailed(e.to_string()))?;
        if !self.is_valid {
            return Err(InstanceClosed);
        }
        let buffer = transform(raw_buffer);
        let mut data = buffer.encode_to_bytes()?;
        let mut target_end = data.len() + mm.len();
        if target_end as u64 > self.file_size {
            // trim the file, drop the duplicate items
            // The encrypt is using stream encryption,
            // subsequent data encryption depends on the results of previous data encryption,
            // we want rewrite the entire stream, so encrypt needs to be reset.
            #[cfg(feature = "encryption")]
            {
                let _ = fs::remove_file(&self.meta_file_path);
                let key = self.encrypt.borrow().key();
                *self.encrypt.borrow_mut() =
                    MmkvImpl::init_encrypt(&self.meta_file_path, key.as_slice());
            }
            let mut vec: Vec<u8> = vec![];
            for buffer in self.kv_map.values() {
                let buffer = transform(buffer.clone());
                vec.extend_from_slice(buffer.encode_to_bytes()?.as_slice());
            }
            // rewrite the entire map
            mm.write_all(vec).map_err(|e| EncodeFailed(e.to_string()))?;
            info!(LOG_TAG, "trimmed, current len: {}", mm.len());
            // the encrypt has been reset, need encode it with new encrypt
            if cfg!(feature = "encryption") {
                data = buffer.encode_to_bytes()?;
            }
            target_end = data.len() + mm.len()
        }
        while target_end as u64 > self.file_size {
            // expand the file size with page_size
            self.file.sync_all().unwrap();
            self.file_size += self.page_size;
            self.file.set_len(self.file_size).unwrap();
            *mm = MemoryMap::new(&self.file);
            info!(LOG_TAG, "expanded, current file size: {}", self.file_size);
        }
        mm.append(data).map_err(|e| EncodeFailed(e.to_string()))?;
        self.kv_map.insert(key.to_string(), buffer.take().unwrap());
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<&Buffer> {
        // acquire read lock
        let _mm = self.mm.read();
        match self.kv_map.get(key) {
            Some(buffer) => Ok(buffer),
            None => Err(Error::KeyNotFound),
        }
    }

    pub fn clear_data(&mut self) {
        // acquire write lock
        let _mm = self.mm.write().unwrap();
        self.kv_map.clear();
        let _ = fs::remove_file(&self.path);
        #[cfg(feature = "encryption")]
        {
            let _ = fs::remove_file(&self.meta_file_path);
        }
        self.is_valid = false;
        info!(LOG_TAG, "data cleared");
    }
}

impl Display for MmkvImpl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MMKV {{ file_size: {}, key_count: {}, content_len: {} }}",
            self.file_size,
            self.kv_map.len(),
            self.mm.read().unwrap().len()
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::OnceLock;
    use std::{fs, thread};

    use crate::core::buffer::Buffer;
    use crate::core::crc::CrcBuffer;
    #[cfg(feature = "encryption")]
    use crate::core::encrypt::EncryptBuffer;
    use crate::core::mmkv_impl::MmkvImpl;
    use crate::Error::{InstanceClosed, KeyNotFound, TypeMissMatch};

    #[test]
    fn test_mmkv_impl() {
        let _ = fs::remove_file("test_mmkv_impl");
        let _ = fs::remove_file("test_mmkv_impl.meta");
        test_mmkv(|| {
            MmkvImpl::new(
                Path::new("test_mmkv_impl"),
                100,
                #[cfg(feature = "encryption")]
                "88C51C536176AD8A8EE4A06F62EE897E",
            )
        });
        let _ = fs::remove_file("test_mmkv_impl");
        let _ = fs::remove_file("test_mmkv_impl.meta");
    }

    fn test_mmkv<F>(f: F)
    where
        F: Fn() -> MmkvImpl,
    {
        let mut mmkv = f();
        mmkv.put("key1", Buffer::from_i32("key1", 1)).unwrap();
        assert_eq!(mmkv.get("key1").unwrap().decode_i32().unwrap(), 1);
        assert_eq!(mmkv.get("key2").is_err(), true);
        mmkv.put("key2", Buffer::from_i32("key2", 2)).unwrap();
        assert_eq!(mmkv.get("key2").unwrap().decode_i32().unwrap(), 2);
        mmkv.put("key3", Buffer::from_i32("key3", 3)).unwrap();
        assert_eq!(mmkv.get("key3").unwrap().decode_i32().unwrap(), 3);

        mmkv.put("key1", Buffer::from_str("key1", "4")).unwrap();
        assert_eq!(mmkv.get("key1").unwrap().decode_i32(), Err(TypeMissMatch));
        assert_eq!(mmkv.get("key1").unwrap().decode_str().unwrap(), "4");

        drop(mmkv);

        let mut mmkv = f();
        assert_eq!(mmkv.get("key1").unwrap().decode_str().unwrap(), "4");
        assert_eq!(mmkv.get("key2").unwrap().decode_i32().unwrap(), 2);
        assert_eq!(mmkv.get("key3").unwrap().decode_i32().unwrap(), 3);

        mmkv.put("key4", Buffer::from_i32("key4", 4)).unwrap();
        mmkv.put("key5", Buffer::from_i32("key5", 5)).unwrap();
        mmkv.put("key6", Buffer::from_i32("key6", 6)).unwrap();
        mmkv.put("key7", Buffer::from_i32("key7", 7)).unwrap();
        mmkv.put("key8", Buffer::from_i32("key8", 8)).unwrap();
        mmkv.put("key9", Buffer::from_i32("key9", 9)).unwrap();
        assert_eq!(mmkv.get("key9").unwrap().decode_i32().unwrap(), 9);
        mmkv.clear_data();
        assert_eq!(mmkv.get("key9"), Err(KeyNotFound));
        let ret = mmkv.put("key9", Buffer::from_i32("key9", 9));
        assert_eq!(ret, Err(InstanceClosed));
        assert_eq!(mmkv.get("key9"), Err(KeyNotFound));
        assert_eq!(mmkv.path.exists(), false);
        #[cfg(feature = "encryption")]
        assert_eq!(mmkv.meta_file_path.exists(), false);
    }

    #[test]
    fn test_multi_thread_mmkv() {
        let _ = fs::remove_file("test_multi_thread_mmkv");
        let _ = fs::remove_file("test_multi_thread_mmkv.meta");
        static mut MMKV: OnceLock<MmkvImpl> = OnceLock::new();
        let mmkv = MmkvImpl::new(
            Path::new("test_multi_thread_mmkv"),
            4096,
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        unsafe {
            MMKV.set(mmkv).unwrap();
            let action = || {
                let current_thread = thread::current();
                let thread_id = current_thread.id();
                for i in 0..500 {
                    let key = &format!("{:?}_key_{i}", thread_id);
                    MMKV.get_mut()
                        .unwrap()
                        .put(key, Buffer::from_i32(key, i))
                        .unwrap();
                }
            };
            thread::scope(|s| {
                for _ in 0..4 {
                    s.spawn(action);
                }
            });
            if cfg!(feature = "encryption") {
                #[cfg(feature = "encryption")]
                {
                    let encrypt = MMKV.get().unwrap().encrypt.clone();
                    let count = MMKV
                        .get()
                        .unwrap()
                        .mm
                        .read()
                        .unwrap()
                        .iter(|| EncryptBuffer::new(encrypt.clone()))
                        .count();
                    assert_eq!(count, 2000);
                }
            } else {
                let count = MMKV
                    .get()
                    .unwrap()
                    .mm
                    .read()
                    .unwrap()
                    .iter(|| CrcBuffer::new())
                    .count();
                assert_eq!(count, 2000);
            };
        }
        let _ = fs::remove_file("test_multi_thread_mmkv");
        let _ = fs::remove_file("test_multi_thread_mmkv.meta");
    }
}
