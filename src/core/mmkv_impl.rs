use crate::core::buffer::{Buffer, Decoder, Encoder};
#[cfg(not(feature = "encryption"))]
use crate::core::crc::CrcEncoderDecoder;
#[cfg(feature = "encryption")]
use crate::core::encrypt::Encrypt;
#[cfg(feature = "encryption")]
use crate::core::encrypt::EncryptEncoderDecoder;
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
use std::time::Instant;

const LOG_TAG: &str = "MMKV:Core";

pub struct MmkvImpl {
    kv_map: HashMap<String, Buffer>,
    lock: RwLock<()>,
    mm: MemoryMap,
    file_size: u64,
    page_size: u64,
    path: PathBuf,
    file: File,
    is_valid: bool,
    encoder: Box<dyn Encoder>,
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
        #[cfg(feature = "encryption")]
        let encoder = Box::new(EncryptEncoderDecoder(encrypt.clone()));
        #[cfg(not(feature = "encryption"))]
        let encoder = Box::new(CrcEncoderDecoder);
        let mut mmkv = MmkvImpl {
            kv_map: HashMap::new(),
            lock: RwLock::new(()),
            mm: MemoryMap::new(&file),
            file_size: file_len,
            page_size,
            path: path.to_path_buf(),
            file,
            is_valid: true,
            encoder,
            #[cfg(feature = "encryption")]
            meta_file_path,
            #[cfg(feature = "encryption")]
            encrypt: encrypt.clone(),
        };
        let time_start = Instant::now();
        mmkv.init();
        let time_end = Instant::now();
        verbose!(
            LOG_TAG,
            "instance initialized, cost {:?}",
            time_end.duration_since(time_start)
        );
        mmkv
    }

    fn init(&mut self) {
        // acquire write lock
        let _lock = self.lock.write().unwrap();
        #[cfg(feature = "encryption")]
        let decoder = EncryptEncoderDecoder(self.encrypt.clone());
        #[cfg(not(feature = "encryption"))]
        let decoder = CrcEncoderDecoder;
        self.mm
            .iter(|bytes| decoder.decode_bytes(bytes))
            .for_each(|buffer: Option<Buffer>| {
                if let Some(data) = buffer {
                    self.kv_map.insert(data.key().to_string(), data);
                }
            });
    }

    #[cfg(feature = "encryption")]
    fn init_encrypt(meta_path: &PathBuf, key: &[u8]) -> Encrypt {
        if meta_path.exists() {
            let mut nonce_file = OpenOptions::new().read(true).open(meta_path).unwrap();
            let mut nonce = Vec::<u8>::new();
            let error_handle = |reason: String| {
                error!(LOG_TAG, "filed to read nonce, reason: {:?}", reason);
                warn!(
                    LOG_TAG, "delete meta file due to previous reason, which may cause mmkv drop all encrypted data"
                );
                let _ = fs::remove_file(meta_path);
                MmkvImpl::init_encrypt(meta_path, key)
            };
            match nonce_file.read_to_end(&mut nonce) {
                Ok(_) => {
                    if nonce.len() != crate::core::encrypt::NONCE_LEN {
                        error_handle("meta file corruption".to_string())
                    } else {
                        Encrypt::new_with_nonce(key.try_into().unwrap(), nonce.as_slice())
                    }
                }
                Err(e) => error_handle(format!("{:?}", e)),
            }
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

    pub fn put(&mut self, key: &str, raw_buffer: Buffer) -> Result<()> {
        // acquire write lock
        let _lock = self.lock.write().map_err(|e| EncodeFailed(e.to_string()))?;
        if !self.is_valid {
            return Err(InstanceClosed);
        }
        let data = self.encoder.encode_to_bytes(&raw_buffer)?;
        self.kv_map.insert(key.to_string(), raw_buffer);
        let target_end = data.len() + self.mm.len();

        if target_end as u64 <= self.file_size {
            self.mm
                .append(data)
                .map_err(|e| EncodeFailed(e.to_string()))?;
            return Ok(());
        }

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
        let time_start = Instant::now();
        info!(LOG_TAG, "start trim, current len {}", self.mm.len());
        let mut count = 0;
        // rewrite the entire map
        self.mm.reset().map_err(|e| EncodeFailed(e.to_string()))?;
        for buffer in self.kv_map.values() {
            let bytes = self.encoder.encode_to_bytes(buffer)?;
            if self.mm.len() + bytes.len() > self.file_size as usize {
                let expand_start = Instant::now();
                info!(LOG_TAG, "start expand, file size: {}", self.file_size);
                // expand the file size with page_size
                self.file.sync_all().unwrap();
                self.file_size += self.page_size;
                self.file.set_len(self.file_size).unwrap();
                self.mm = MemoryMap::new(&self.file);
                let expand_end = Instant::now();
                info!(
                    LOG_TAG,
                    "expanded, file size: {}, cost {:?}",
                    self.file_size,
                    expand_end.duration_since(expand_start)
                );
            }
            self.mm
                .append(bytes)
                .map_err(|e| EncodeFailed(e.to_string()))?;
            count += 1;
        }
        let time_end = Instant::now();
        info!(
            LOG_TAG,
            "wrote {} items, new len {}, cost {:?}",
            count,
            self.mm.len(),
            time_end.duration_since(time_start)
        );
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<&Buffer> {
        // acquire read lock
        let _lock = self.lock.read().map_err(|e| EncodeFailed(e.to_string()))?;
        match self.kv_map.get(key) {
            Some(buffer) => Ok(buffer),
            None => Err(Error::KeyNotFound),
        }
    }

    pub fn clear_data(&mut self) {
        // acquire write lock
        let _lock = self.lock.write();
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
            self.mm.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::atomic::{AtomicPtr, Ordering};
    use std::{fs, thread};

    use crate::core::buffer::Buffer;
    use crate::core::mmkv_impl::MmkvImpl;
    use crate::Error::{InstanceClosed, KeyNotFound, TypeMissMatch};

    #[test]
    fn test_trim_and_expand() {
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
        #[cfg(feature = "encryption")]
        assert_eq!(mmkv.mm.len(), 32);
        #[cfg(not(feature = "encryption"))]
        assert_eq!(mmkv.mm.len(), 25);
        assert_eq!(mmkv.get("key1").unwrap().decode_i32().unwrap(), 1);
        mmkv.put("key2", Buffer::from_i32("key2", 2)).unwrap();
        #[cfg(feature = "encryption")]
        assert_eq!(mmkv.mm.len(), 56);
        #[cfg(not(feature = "encryption"))]
        assert_eq!(mmkv.mm.len(), 42);
        assert_eq!(mmkv.get("key2").unwrap().decode_i32().unwrap(), 2);
        mmkv.put("key3", Buffer::from_i32("key3", 3)).unwrap();
        #[cfg(feature = "encryption")]
        assert_eq!(mmkv.mm.len(), 80);
        #[cfg(not(feature = "encryption"))]
        assert_eq!(mmkv.mm.len(), 59);
        assert_eq!(mmkv.get("key3").unwrap().decode_i32().unwrap(), 3);
        mmkv.put("key1", Buffer::from_str("key1", "4")).unwrap();
        #[cfg(feature = "encryption")]
        // put str "4", which len 23, current file size 100, trim here, no duplicate item, expand
        // file to 200
        assert_eq!(mmkv.mm.len(), 79);
        #[cfg(not(feature = "encryption"))]
        assert_eq!(mmkv.mm.len(), 75);
        assert_eq!(mmkv.get("key1").unwrap().decode_i32(), Err(TypeMissMatch));
        assert_eq!(mmkv.get("key1").unwrap().decode_str().unwrap(), "4");

        drop(mmkv);

        let mut mmkv = f();
        assert_eq!(mmkv.get("key1").unwrap().decode_str().unwrap(), "4");
        assert_eq!(mmkv.get("key2").unwrap().decode_i32().unwrap(), 2);
        assert_eq!(mmkv.get("key3").unwrap().decode_i32().unwrap(), 3);

        mmkv.put("key4", Buffer::from_i32("key4", 4)).unwrap();
        #[cfg(feature = "encryption")]
        assert_eq!(mmkv.mm.len(), 103);
        #[cfg(feature = "encryption")]
        assert_eq!(mmkv.file_size, 200);
        #[cfg(not(feature = "encryption"))]
        assert_eq!(mmkv.mm.len(), 92);
        mmkv.put("key5", Buffer::from_i32("key5", 5)).unwrap();
        #[cfg(feature = "encryption")]
        assert_eq!(mmkv.mm.len(), 127);
        #[cfg(not(feature = "encryption"))]
        // trim here, drop one item
        assert_eq!(mmkv.mm.len(), 92);
        mmkv.put("key6", Buffer::from_i32("key6", 6)).unwrap();
        #[cfg(feature = "encryption")]
        assert_eq!(mmkv.mm.len(), 151);
        #[cfg(not(feature = "encryption"))]
        // trim here, no duplicate item, expand file to 200
        assert_eq!(mmkv.mm.len(), 109);
        #[cfg(not(feature = "encryption"))]
        assert_eq!(mmkv.file_size, 200);
        mmkv.put("key7", Buffer::from_i32("key7", 7)).unwrap();
        #[cfg(feature = "encryption")]
        assert_eq!(mmkv.mm.len(), 175);
        #[cfg(not(feature = "encryption"))]
        assert_eq!(mmkv.mm.len(), 126);
        mmkv.put("key8", Buffer::from_i32("key8", 8)).unwrap();
        #[cfg(feature = "encryption")]
        assert_eq!(mmkv.mm.len(), 199);
        #[cfg(not(feature = "encryption"))]
        assert_eq!(mmkv.mm.len(), 143);
        mmkv.put("key9", Buffer::from_i32("key9", 9)).unwrap();
        #[cfg(feature = "encryption")]
        // put 9, trim here, no duplicate item, expand file to 300
        assert_eq!(mmkv.mm.len(), 223);
        #[cfg(feature = "encryption")]
        assert_eq!(mmkv.file_size, 300);
        #[cfg(not(feature = "encryption"))]
        assert_eq!(mmkv.mm.len(), 160);
        assert_eq!(mmkv.get("key9").unwrap().decode_i32().unwrap(), 9);
        mmkv.clear_data();
        assert_eq!(mmkv.get("key9"), Err(KeyNotFound));
        let ret = mmkv.put("key9", Buffer::from_i32("key9", 9));
        assert_eq!(ret, Err(InstanceClosed));
        assert_eq!(mmkv.get("key9"), Err(KeyNotFound));
        assert!(!mmkv.path.exists());
        #[cfg(feature = "encryption")]
        assert!(!mmkv.meta_file_path.exists());
    }

    #[test]
    fn test_multi_thread_mmkv() {
        let _ = fs::remove_file("test_multi_thread_mmkv");
        let _ = fs::remove_file("test_multi_thread_mmkv.meta");
        let mmkv_impl = MmkvImpl::new(
            Path::new("test_multi_thread_mmkv"),
            4096,
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        let mmkv_ptr = Box::into_raw(Box::new(mmkv_impl));
        let mmkv = AtomicPtr::new(std::ptr::null_mut());
        mmkv.store(mmkv_ptr, Ordering::Release);
        let action = |thread_id: &str| {
            for i in 0..1000 {
                let key = &format!("{thread_id}_key_{i}");
                let mmkv = unsafe { mmkv.load(Ordering::Acquire).as_mut().unwrap() };
                mmkv.put(key, Buffer::from_i32(key, i)).unwrap();
            }
        };
        thread::scope(|s| {
            for i in 0..4 {
                s.spawn(move || action(format!("thread_{i}").as_ref()));
            }
        });
        unsafe {
            let _ = Box::from_raw(mmkv.swap(std::ptr::null_mut(), Ordering::Release));
        }
        let mmkv = MmkvImpl::new(
            Path::new("test_multi_thread_mmkv"),
            4096,
            #[cfg(feature = "encryption")]
            "88C51C536176AD8A8EE4A06F62EE897E",
        );
        for i in 0..4 {
            for j in 0..1000 {
                let key = &format!("thread_{i}_key_{j}");
                assert_eq!(mmkv.get(key).unwrap().decode_i32().unwrap(), j)
            }
        }
        let _ = fs::remove_file("test_multi_thread_mmkv");
        let _ = fs::remove_file("test_multi_thread_mmkv.meta");
    }
}
