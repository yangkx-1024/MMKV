use crate::core::buffer::{Buffer, Encoder, Take};
use crate::core::crc::CrcBuffer;
#[cfg(feature = "encryption")]
use crate::core::encrypt::{Encrypt, EncryptBuffer};
use crate::core::memory_map::MemoryMap;
#[cfg(feature = "encryption")]
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
#[cfg(feature = "encryption")]
use std::fs;
use std::fs::{File, OpenOptions};
#[cfg(feature = "encryption")]
use std::io::{Read, Write};
use std::path::Path;
#[cfg(feature = "encryption")]
use std::path::PathBuf;
#[cfg(feature = "encryption")]
use std::rc::Rc;
use std::sync::RwLock;

#[derive(Debug)]
pub struct MmkvImpl {
    kv_map: HashMap<String, Buffer>,
    file: File,
    #[cfg(feature = "encryption")]
    meta_file_path: PathBuf,
    mm: RwLock<MemoryMap>,
    file_size: u64,
    page_size: u64,
    #[cfg(feature = "encryption")]
    crypt: Rc<RefCell<Encrypt>>,
}

impl MmkvImpl {
    pub fn new(path: &Path, page_size: u64, #[cfg(feature = "encryption")] key: &str) -> Self {
        MmkvImpl::new_mmkv(
            path,
            page_size,
            #[cfg(feature = "encryption")]
            key,
        )
    }

    fn new_mmkv(path: &Path, page_size: u64, #[cfg(feature = "encryption")] key: &str) -> Self {
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
        let crypt = {
            let key = hex::decode(key).unwrap();
            Rc::new(RefCell::new(MmkvImpl::init_crypt(
                &meta_file_path,
                key.as_slice(),
            )))
        };
        let mm = MemoryMap::new(&file);
        let mut mmkv = MmkvImpl {
            kv_map: HashMap::new(),
            file,
            #[cfg(feature = "encryption")]
            meta_file_path,
            mm: RwLock::new(mm),
            file_size: file_len,
            page_size,
            #[cfg(feature = "encryption")]
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
        if cfg!(feature = "encryption") {
            #[cfg(feature = "encryption")]
            mm.iter(|| EncryptBuffer::new(self.crypt.clone()))
                .for_each(decode)
        } else {
            mm.iter(|| CrcBuffer::new()).for_each(decode)
        }
    }

    #[cfg(feature = "encryption")]
    fn init_crypt(meta_path: &PathBuf, key: &[u8]) -> Encrypt {
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
            nonce_file.sync_all().unwrap();
            crypt
        }
    }

    pub fn write(&mut self, key: &str, buffer: Buffer) {
        if cfg!(feature = "encryption") {
            #[cfg(feature = "encryption")]
            let crypt = self.crypt.clone();
            #[cfg(feature = "encryption")]
            self.write_internal(key, buffer, |buffer: Buffer| {
                EncryptBuffer::new_with_buffer(buffer, crypt.clone())
            })
        } else {
            self.write_internal(key, buffer, |buffer: Buffer| {
                CrcBuffer::new_with_buffer(buffer)
            })
        };
    }

    fn write_internal<T, F>(&mut self, key: &str, raw_buffer: Buffer, transform: F)
    where
        T: Encoder + Take,
        F: Fn(Buffer) -> T,
    {
        // acquire write lock
        let mut mm = self.mm.write().unwrap();
        let buffer = transform(raw_buffer);
        let mut data = buffer.encode_to_bytes();
        let mut target_end = data.len() + mm.len();
        if target_end as u64 > self.file_size {
            // trim the file, drop the duplicate items
            // The crypt is using stream encryption,
            // subsequent data encryption depends on the results of previous data encryption,
            // we want rewrite the entire stream, so crypt needs to be reset.
            #[cfg(feature = "encryption")]
            {
                let _ = fs::remove_file(self.meta_file_path.to_path_buf());
                let key = self.crypt.borrow().key();
                *self.crypt.borrow_mut() =
                    MmkvImpl::init_crypt(&self.meta_file_path, key.as_slice());
            }
            let mut vec: Vec<u8> = vec![];
            for buffer in self.kv_map.values() {
                let buffer = transform(buffer.clone());
                vec.extend_from_slice(buffer.encode_to_bytes().as_slice());
            }
            // rewrite the entire map
            mm.write_all(vec).unwrap();
            // the crypt has been reset, need encode it with new crypt
            if cfg!(feature = "encryption") {
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
    use std::path::Path;
    use std::sync::OnceLock;
    use std::thread::{spawn, JoinHandle};
    use std::{fs, thread};

    use crate::core::buffer::Buffer;
    use crate::core::crc::CrcBuffer;
    #[cfg(feature = "encryption")]
    use crate::core::encrypt::EncryptBuffer;
    use crate::core::mmkv_impl::MmkvImpl;

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
            if cfg!(feature = "encryption") {
                #[cfg(feature = "encryption")]
                {
                    let crypt = MMKV.get().unwrap().crypt.clone();
                    let count = MMKV
                        .get()
                        .unwrap()
                        .mm
                        .read()
                        .unwrap()
                        .iter(|| EncryptBuffer::new(crypt.clone()))
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
