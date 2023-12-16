use crate::core::buffer::{Buffer, Decoder, Encoder};
use crate::core::config::Config;
#[cfg(not(feature = "encryption"))]
use crate::core::crc::CrcEncoderDecoder;
#[cfg(feature = "encryption")]
use crate::core::encrypt::Encryptor;
use crate::core::io_looper::IOLooper;
use crate::core::memory_map::MemoryMap;
use crate::Error::{EncodeFailed, InstanceClosed};
use crate::{Error, Result};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::RwLock;
use std::time::Instant;

const LOG_TAG: &str = "MMKV:Core";

pub struct MmkvImpl {
    ptr: &'static AtomicPtr<MmkvImpl>,
    kv_map: RwLock<HashMap<String, Buffer>>,
    mm: MemoryMap,
    config: Config,
    is_valid: bool,
    encoder: Box<dyn Encoder>,
    io_looper: Option<IOLooper>,
    #[cfg(feature = "encryption")]
    encryptor: Encryptor,
}

impl MmkvImpl {
    fn drop_instance(ptr: &AtomicPtr<MmkvImpl>) {
        let instance = ptr.load(Ordering::Acquire);
        if !instance.is_null() {
            let time_start = Instant::now();
            unsafe {
                let mut mmkv = Box::from_raw(instance);
                drop(mmkv.io_looper.take());
            };
            ptr.store(std::ptr::null_mut(), Ordering::Release);
            info!(
                LOG_TAG,
                "mmkv dropped, cost {:?}",
                Instant::now().duration_since(time_start)
            );
        }
    }

    pub fn init(
        ptr: &'static AtomicPtr<MmkvImpl>,
        config: Config,
        #[cfg(feature = "encryption")] key: &str,
    ) {
        MmkvImpl::drop_instance(ptr);
        let time_start = Instant::now();
        let mmkv_impl = MmkvImpl::new(
            ptr,
            config,
            #[cfg(feature = "encryption")]
            key,
        );
        ptr.store(Box::into_raw(Box::new(mmkv_impl)), Ordering::Release);
        info!(
            LOG_TAG,
            "instance initialized, cost {:?}",
            Instant::now().duration_since(time_start)
        );
    }

    fn new(
        ptr: &'static AtomicPtr<MmkvImpl>,
        config: Config,
        #[cfg(feature = "encryption")] key: &str,
    ) -> Self {
        #[cfg(feature = "encryption")]
        let encryptor = Encryptor::init(&config.path, key);
        #[cfg(feature = "encryption")]
        let encoder = Box::new(encryptor.clone());
        #[cfg(not(feature = "encryption"))]
        let encoder = Box::new(CrcEncoderDecoder);
        let mut mmkv = MmkvImpl {
            ptr,
            kv_map: RwLock::new(HashMap::new()),
            mm: MemoryMap::new(&config.file),
            config,
            is_valid: true,
            encoder,
            io_looper: Some(IOLooper::new()),
            #[cfg(feature = "encryption")]
            encryptor,
        };
        mmkv.load_data();
        mmkv
    }

    fn load_data(&mut self) {
        // acquire write lock
        let mut kv_map = self.kv_map.write().unwrap();
        #[cfg(feature = "encryption")]
        let decoder = self.encryptor.clone();
        #[cfg(not(feature = "encryption"))]
        let decoder = CrcEncoderDecoder;
        self.mm
            .iter(|bytes| decoder.decode_bytes(bytes))
            .for_each(|buffer: Option<Buffer>| {
                if let Some(data) = buffer {
                    kv_map.insert(data.key().to_string(), data);
                }
            });
    }

    pub fn put(&mut self, key: &str, raw_buffer: Buffer) -> Result<()> {
        let mut kv_map = self.kv_map.write().unwrap();
        if !self.is_valid {
            return Err(InstanceClosed);
        }
        kv_map.insert(key.to_string(), raw_buffer.clone());
        // Ref to current instance, which will wait io thread to finish before been dropped.
        // So it's safe to send it to io thread
        let mmkv = unsafe { self.ptr.load(Ordering::Acquire).as_mut() }.unwrap();
        self.io_looper.as_ref().unwrap().execute(move || {
            mmkv.flash(raw_buffer);
        })
    }

    /**
    Flash the data to file, always running in one io thread, so don't need lock here
     */
    fn flash(&mut self, buffer: Buffer) {
        let data = self.encoder.encode_to_bytes(&buffer).unwrap();
        let target_end = data.len() + self.mm.len();
        let file_size = self.config.file_size();
        if target_end as u64 <= file_size {
            self.mm.append(data).unwrap();
        } else {
            let kv_map = { self.kv_map.read().unwrap().clone() };
            #[cfg(feature = "encryption")]
            self.encryptor.reset();
            let time_start = Instant::now();
            info!(LOG_TAG, "start trim, current len {}", self.mm.len());
            let mut count = 0;
            // rewrite the entire map
            self.mm
                .reset()
                .map_err(|e| EncodeFailed(e.to_string()))
                .unwrap();
            for buffer in kv_map.values() {
                let bytes = self.encoder.encode_to_bytes(buffer).unwrap();
                if self.mm.len() + bytes.len() > file_size as usize {
                    self.config.expand();
                    self.mm = MemoryMap::new(&self.config.file);
                }
                self.mm.append(bytes).unwrap();
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
        }
    }

    pub fn get(&self, key: &str) -> Result<Buffer> {
        match self.kv_map.read().unwrap().get(key) {
            Some(buffer) => Ok(buffer.clone()),
            None => Err(Error::KeyNotFound),
        }
    }

    pub fn clear_data(&mut self) {
        // acquire write lock
        let mut kv_map = self.kv_map.write().unwrap();
        if !self.is_valid {
            warn!(LOG_TAG, "instance already closed");
        }
        self.is_valid = false;
        kv_map.clear();
        if let Some(mut looper) = self.io_looper.take() {
            looper.kill()
        }
        self.config.remove_file();
        #[cfg(feature = "encryption")]
        self.encryptor.remove_file();
        MmkvImpl::drop_instance(self.ptr);
        info!(LOG_TAG, "data cleared");
    }

    pub fn close(&mut self) {
        // acquire write lock
        let _lock = self.kv_map.write().unwrap();
        self.is_valid = false;
        MmkvImpl::drop_instance(self.ptr);
        info!(LOG_TAG, "instance closed");
    }
}

impl Display for MmkvImpl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MMKV {{ file_size: {}, key_count: {}, content_len: {} }}",
            self.config.file_size(),
            self.kv_map.read().unwrap().len(),
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
    use crate::core::config::Config;
    use crate::core::mmkv_impl::MmkvImpl;
    use crate::LogLevel::Debug;
    use crate::MMKV;

    #[cfg(feature = "encryption")]
    const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

    macro_rules! mmkv {
        ($ptr:expr) => {
            unsafe { $ptr.load(Ordering::Acquire).as_mut() }.unwrap()
        };
    }

    macro_rules! init {
        ($ptr:expr, $config:expr) => {{
            MMKV::set_log_level(Debug);
            MmkvImpl::init(
                $ptr,
                $config,
                #[cfg(feature = "encryption")]
                TEST_KEY,
            );
        }};
    }

    #[test]
    #[cfg(not(feature = "encryption"))]
    fn test_trim_and_expand_default() {
        let file_path = "test_trim_and_expand_default";
        let _ = fs::remove_file(file_path);
        let _ = fs::remove_file(format!("{}.meta", file_path));
        static PTR: AtomicPtr<MmkvImpl> = AtomicPtr::new(std::ptr::null_mut());
        let config = Config::new(Path::new(file_path), 100);
        init!(&PTR, config.clone());
        mmkv!(&PTR)
            .put("key1", Buffer::from_i32("key1", 1))
            .unwrap(); // + 17
        init!(&PTR, config.clone());
        assert_eq!(mmkv!(&PTR).mm.len(), 25);
        mmkv!(&PTR)
            .put("key2", Buffer::from_i32("key2", 2))
            .unwrap(); // + 17
        mmkv!(&PTR)
            .put("key3", Buffer::from_i32("key3", 3))
            .unwrap(); // + 17
        mmkv!(&PTR)
            .put("key1", Buffer::from_i32("key1", 4))
            .unwrap(); // + 17
        mmkv!(&PTR)
            .put("key2", Buffer::from_i32("key2", 5))
            .unwrap(); // + 17
        init!(&PTR, config.clone());
        assert_eq!(mmkv!(&PTR).mm.len(), 93);
        mmkv!(&PTR)
            .put("key1", Buffer::from_i32("key1", 6))
            .unwrap(); // + 17, trim, 3 items remain
        init!(&PTR, config.clone());
        assert_eq!(mmkv!(&PTR).mm.len(), 59);
        assert_eq!(mmkv!(&PTR).get("key1").unwrap().decode_i32(), Ok(6));
        assert_eq!(mmkv!(&PTR).get("key2").unwrap().decode_i32(), Ok(5));
        mmkv!(&PTR)
            .put("key4", Buffer::from_i32("key4", 4))
            .unwrap();
        mmkv!(&PTR)
            .put("key5", Buffer::from_i32("key5", 5))
            .unwrap(); // 93
        mmkv!(&PTR)
            .put("key6", Buffer::from_i32("key6", 6))
            .unwrap(); // expand, 110
        init!(&PTR, config.clone());
        assert_eq!(mmkv!(&PTR).mm.len(), 110);
        assert_eq!(mmkv!(&PTR).config.file_size(), 200);
        mmkv!(&PTR)
            .put("key7", Buffer::from_i32("key7", 7))
            .unwrap();
        init!(&PTR, config);
        assert_eq!(mmkv!(&PTR).mm.len(), 127);
        mmkv!(&PTR).clear_data();
        assert!(PTR.load(Ordering::Acquire).is_null());
    }

    #[test]
    #[cfg(feature = "encryption")]
    fn test_trim_and_expand_encrypt() {
        let file = "test_trim_and_expand_encrypt";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{file}.meta"));
        static PTR: AtomicPtr<MmkvImpl> = AtomicPtr::new(std::ptr::null_mut());
        let config = Config::new(Path::new(file), 100);
        init!(&PTR, config.clone());
        mmkv!(&PTR)
            .put("key1", Buffer::from_i32("key1", 1))
            .unwrap(); // + 24
        init!(&PTR, config.clone());
        assert_eq!(mmkv!(&PTR).mm.len(), 32);
        mmkv!(&PTR)
            .put("key2", Buffer::from_i32("key2", 2))
            .unwrap(); // + 24
        mmkv!(&PTR)
            .put("key3", Buffer::from_i32("key3", 3))
            .unwrap(); // + 24
        init!(&PTR, config.clone());
        assert_eq!(mmkv!(&PTR).mm.len(), 80);
        mmkv!(&PTR)
            .put("key1", Buffer::from_i32("key1", 4))
            .unwrap(); // + 24 trim
        mmkv!(&PTR)
            .put("key2", Buffer::from_i32("key2", 5))
            .unwrap(); // + 24 trim
        init!(&PTR, config.clone());
        assert_eq!(mmkv!(&PTR).mm.len(), 80);
        assert_eq!(mmkv!(&PTR).get("key1").unwrap().decode_i32(), Ok(4));
        assert_eq!(mmkv!(&PTR).get("key2").unwrap().decode_i32(), Ok(5));
        mmkv!(&PTR)
            .put("key4", Buffer::from_i32("key4", 4))
            .unwrap(); // + 24
        init!(&PTR, config.clone());
        assert_eq!(mmkv!(&PTR).mm.len(), 104);
        assert_eq!(mmkv!(&PTR).config.file_size(), 200);
        mmkv!(&PTR)
            .put("key5", Buffer::from_i32("key5", 5))
            .unwrap(); // + 24
        init!(&PTR, config.clone());
        assert_eq!(mmkv!(&PTR).mm.len(), 128);
        mmkv!(&PTR).clear_data();
        assert!(PTR.load(Ordering::Acquire).is_null());
    }

    #[test]
    fn test_multi_thread_mmkv() {
        let file = "test_multi_thread_mmkv";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{}.meta", file));
        static PTR: AtomicPtr<MmkvImpl> = AtomicPtr::new(std::ptr::null_mut());
        let config = Config::new(Path::new(file), 4096);
        let loop_count = 1000;
        init!(&PTR, config.clone());
        let action = |thread_id: &str| {
            let mmkv = mmkv!(&PTR);
            for i in 0..loop_count {
                let key = &format!("{thread_id}_key_{i}");
                mmkv.put(key, Buffer::from_i32(key, i)).unwrap();
            }
        };
        thread::scope(|s| {
            for i in 0..2 {
                s.spawn(move || action(format!("thread_{i}").as_ref()));
            }
        });
        mmkv!(&PTR).close();
        init!(&PTR, config);
        let mmkv = mmkv!(&PTR);
        for i in 0..2 {
            for j in 0..loop_count {
                let key = &format!("thread_{i}_key_{j}");
                assert_eq!(mmkv.get(key).unwrap().decode_i32().unwrap(), j)
            }
        }
        mmkv.clear_data();
        assert!(PTR.load(Ordering::Acquire).is_null());
    }
}
