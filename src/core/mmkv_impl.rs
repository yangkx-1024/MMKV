use crate::core::buffer::{Buffer, Decoder, Encoder};
use crate::core::config::Config;
#[cfg(not(feature = "encryption"))]
use crate::core::crc::CrcEncoderDecoder;
#[cfg(feature = "encryption")]
use crate::core::encrypt::Encryptor;
use crate::core::io_looper::{Callback, IOLooper};
use crate::core::memory_map::MemoryMap;
use crate::Error::{EncodeFailed, InstanceClosed};
use crate::{Error, Result};
use std::collections::HashMap;
use std::time::Instant;

const LOG_TAG: &str = "MMKV:Core";

pub struct MmkvImpl {
    kv_map: HashMap<String, Buffer>,
    is_valid: bool,
    io_looper: Option<IOLooper>,
}

impl Drop for MmkvImpl {
    fn drop(&mut self) {
        let time_start = Instant::now();
        drop(self.io_looper.take());
        debug!(
            LOG_TAG,
            "wait for io task finish, cost {:?}",
            time_start.elapsed()
        );
    }
}

struct IOWriter {
    config: Config,
    mm: MemoryMap,
    need_trim: bool,
    encoder: Box<dyn Encoder>,
    #[cfg(feature = "encryption")]
    encryptor: Encryptor,
}

impl Callback for IOWriter {}

impl IOWriter {
    fn new(
        config: Config,
        mm: MemoryMap,
        encoder: Box<dyn Encoder>,
        #[cfg(feature = "encryption")] encryptor: Encryptor,
    ) -> Self {
        IOWriter {
            config,
            mm,
            need_trim: false,
            encoder,
            #[cfg(feature = "encryption")]
            encryptor,
        }
    }

    // Flash the data to file, always running in one io thread, so don't need lock here
    fn write(&mut self, buffer: Buffer, map: HashMap<String, Buffer>, duplicated: bool) {
        let data = self.encoder.encode_to_bytes(&buffer).unwrap();
        let target_end = data.len() + self.mm.len();
        let file_size = self.config.file_size();
        if duplicated {
            self.need_trim = true;
        }
        if target_end as u64 <= file_size {
            self.mm.append(data).unwrap();
            return;
        }
        if self.need_trim {
            // rewrite the entire map
            #[cfg(feature = "encryption")]
            self.encryptor.reset();
            let time_start = Instant::now();
            info!(LOG_TAG, "start trim, current len {}", self.mm.len());
            let mut count = 0;
            self.mm
                .reset()
                .map_err(|e| EncodeFailed(e.to_string()))
                .unwrap();
            for buffer in map.values() {
                let bytes = self.encoder.encode_to_bytes(buffer).unwrap();
                if self.mm.len() + bytes.len() > file_size as usize {
                    self.expand();
                }
                self.mm.append(bytes).unwrap();
                count += 1;
            }
            self.need_trim = false;
            info!(
                LOG_TAG,
                "wrote {} items, new len {}, cost {:?}",
                count,
                self.mm.len(),
                time_start.elapsed()
            );
        } else {
            // expand and write
            self.expand();
            self.mm.append(data).unwrap();
        }
    }

    fn expand(&mut self) {
        self.config.expand();
        self.mm = MemoryMap::new(&self.config.file);
    }

    fn remove_file(&mut self) {
        self.config.remove_file();
        #[cfg(feature = "encryption")]
        self.encryptor.remove_file();
    }
}

impl MmkvImpl {
    pub fn new(config: Config, #[cfg(feature = "encryption")] key: &str) -> Self {
        let time_start = Instant::now();
        #[cfg(feature = "encryption")]
        let encryptor = Encryptor::init(&config.path, key);
        #[cfg(feature = "encryption")]
        let encoder = Box::new(encryptor.clone());
        #[cfg(not(feature = "encryption"))]
        let encoder = Box::new(CrcEncoderDecoder);
        let mut kv_map = HashMap::new();
        let mm = MemoryMap::new(&config.file);
        #[cfg(feature = "encryption")]
        let decoder = encryptor.clone();
        #[cfg(not(feature = "encryption"))]
        let decoder = CrcEncoderDecoder;
        mm.iter(|bytes| decoder.decode_bytes(bytes))
            .for_each(|buffer: Option<Buffer>| {
                if let Some(data) = buffer {
                    kv_map.insert(data.key().to_string(), data);
                }
            });
        let io_writer = IOWriter::new(
            config,
            mm,
            encoder,
            #[cfg(feature = "encryption")]
            encryptor,
        );
        let content_len = io_writer.mm.len();
        let mmkv = MmkvImpl {
            kv_map,
            is_valid: true,
            io_looper: Some(IOLooper::new(io_writer)),
        };
        info!(
            LOG_TAG,
            "instance initialized, read {} items, content len {}, cost {:?}",
            mmkv.kv_map.len(),
            content_len,
            time_start.elapsed()
        );
        mmkv
    }

    pub fn put(&mut self, key: &str, raw_buffer: Buffer) -> Result<()> {
        if !self.is_valid {
            return Err(InstanceClosed);
        }
        let result = self.kv_map.insert(key.to_string(), raw_buffer.clone());
        let duplicated = result.is_some();
        let map = self.kv_map.clone();
        self.io_looper.as_ref().unwrap().post(move |callback| {
            callback
                .downcast_mut::<IOWriter>()
                .unwrap()
                .write(raw_buffer, map, duplicated)
        })
    }

    pub fn get(&self, key: &str) -> Result<&Buffer> {
        if !self.is_valid {
            return Err(InstanceClosed);
        }
        match self.kv_map.get(key) {
            Some(buffer) => Ok(buffer),
            None => Err(Error::KeyNotFound),
        }
    }

    pub fn clear_data(&mut self) {
        if !self.is_valid {
            warn!(LOG_TAG, "instance already closed");
            return;
        }
        self.is_valid = false;
        self.kv_map.clear();
        self.io_looper.as_mut().unwrap().post_and_kill(|callback| {
            callback.downcast_mut::<IOWriter>().unwrap().remove_file();
            info!(LOG_TAG, "data cleared");
        });
    }

    pub fn close(&mut self) {
        if !self.is_valid {
            warn!(LOG_TAG, "instance already closed");
            return;
        }
        self.is_valid = false;
        self.kv_map.clear();
        info!(LOG_TAG, "instance closed");
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::RwLock;
    use std::{fs, thread};

    use crate::core::buffer::Buffer;
    use crate::core::config::Config;
    use crate::core::mmkv_impl::IOWriter;
    use crate::core::mmkv_impl::MmkvImpl;
    use crate::LogLevel::Debug;
    use crate::MMKV;

    #[cfg(feature = "encryption")]
    const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

    fn init(config: &Config) -> MmkvImpl {
        MMKV::set_log_level(Debug);
        MmkvImpl::new(
            config.clone(),
            #[cfg(feature = "encryption")]
            TEST_KEY,
        )
    }

    macro_rules! assert_mmkv_len {
        ($mmkv:expr, $value:literal) => {
            $mmkv
                .io_looper
                .as_ref()
                .unwrap()
                .post(|writer| {
                    assert_eq!(writer.downcast_ref::<IOWriter>().unwrap().mm.len(), $value);
                })
                .expect("");
        };
    }

    #[test]
    #[cfg(not(feature = "encryption"))]
    fn test_trim_and_expand_default() {
        let file_path = "test_trim_and_expand_default";
        let _ = fs::remove_file(file_path);
        assert!(!Path::new(file_path).exists());
        let _ = fs::remove_file(format!("{}.meta", file_path));
        let config = &Config::new(Path::new(file_path), 100);
        let mut mmkv = init(config);
        mmkv.put("key1", Buffer::from_i32("key1", 1)).unwrap(); // + 17
        drop(mmkv);
        mmkv = init(config);
        assert_mmkv_len!(mmkv, 25);
        mmkv.put("key2", Buffer::from_i32("key2", 2)).unwrap(); // + 17
        mmkv.put("key3", Buffer::from_i32("key3", 3)).unwrap(); // + 17
        mmkv.put("key1", Buffer::from_i32("key1", 4)).unwrap(); // + 17
        mmkv.put("key2", Buffer::from_i32("key2", 5)).unwrap(); // + 17
        drop(mmkv);
        mmkv = init(config);
        assert_mmkv_len!(mmkv, 93);
        mmkv.put("key1", Buffer::from_i32("key1", 6)).unwrap(); // + 17, trim, 3 items remain
        drop(mmkv);
        mmkv = init(config);
        assert_mmkv_len!(mmkv, 59);
        assert_eq!(mmkv.get("key1").unwrap().decode_i32(), Ok(6));
        assert_eq!(mmkv.get("key2").unwrap().decode_i32(), Ok(5));
        mmkv.put("key4", Buffer::from_i32("key4", 4)).unwrap();
        mmkv.put("key5", Buffer::from_i32("key5", 5)).unwrap(); // 93
        mmkv.put("key6", Buffer::from_i32("key6", 6)).unwrap(); // expand, 110
        drop(mmkv);
        mmkv = init(config);
        assert_mmkv_len!(mmkv, 110);
        assert_eq!(config.file_size(), 200);
        mmkv.put("key7", Buffer::from_i32("key7", 7)).unwrap();
        drop(mmkv);
        mmkv = init(config);
        assert_mmkv_len!(mmkv, 127);
        mmkv.clear_data();
        assert!(!Path::new(file_path).exists());
    }

    #[test]
    #[cfg(feature = "encryption")]
    fn test_trim_and_expand_encrypt() {
        let file = "test_trim_and_expand_encrypt";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{file}.meta"));
        let config = &Config::new(Path::new(file), 100);
        let mut mmkv = init(config);
        mmkv.put("key1", Buffer::from_i32("key1", 1)).unwrap(); // + 24
        drop(mmkv);
        mmkv = init(config);
        assert_mmkv_len!(mmkv, 32);
        mmkv.put("key2", Buffer::from_i32("key2", 2)).unwrap(); // + 24
        mmkv.put("key3", Buffer::from_i32("key3", 3)).unwrap(); // + 24
        drop(mmkv);
        mmkv = init(config);
        assert_mmkv_len!(mmkv, 80);
        mmkv.put("key1", Buffer::from_i32("key1", 4)).unwrap(); // + 24 trim
        mmkv.put("key2", Buffer::from_i32("key2", 5)).unwrap(); // + 24 trim
        drop(mmkv);
        mmkv = init(config);
        assert_mmkv_len!(mmkv, 80);
        assert_eq!(mmkv.get("key1").unwrap().decode_i32(), Ok(4));
        assert_eq!(mmkv.get("key2").unwrap().decode_i32(), Ok(5));
        mmkv.put("key4", Buffer::from_i32("key4", 4)).unwrap(); // + 24
        drop(mmkv);
        mmkv = init(config);
        assert_mmkv_len!(mmkv, 104);
        assert_eq!(config.file_size(), 200);
        mmkv.put("key5", Buffer::from_i32("key5", 5)).unwrap(); // + 24
        drop(mmkv);
        mmkv = init(config);
        assert_mmkv_len!(mmkv, 128);
        mmkv.clear_data();
        assert!(!Path::new(file).exists());
    }

    #[test]
    fn test_multi_thread_mmkv() {
        let file = "test_multi_thread_mmkv";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{}.meta", file));
        let config = &Config::new(Path::new(file), 4096);
        let mmkv = RwLock::new(Some(init(config)));
        let loop_count = 1000;
        let action = |thread_id: &str| {
            for i in 0..loop_count {
                let key = &format!("{thread_id}_key_{i}");
                mmkv.write()
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .put(key, Buffer::from_i32(key, i))
                    .unwrap();
            }
        };
        thread::scope(|s| {
            s.spawn(|| {
                let repeat_key = "test_multi_thread_mmkv_repeat_key";
                for i in 0..loop_count {
                    mmkv.write()
                        .unwrap()
                        .as_mut()
                        .unwrap()
                        .put(repeat_key, Buffer::from_i32(repeat_key, i))
                        .unwrap();
                }
            });
            for i in 0..2 {
                s.spawn(move || action(format!("thread_{i}").as_ref()));
            }
        });
        drop(mmkv.write().unwrap().take());
        *mmkv.write().unwrap() = Some(init(config));
        for i in 0..2 {
            for j in 0..loop_count {
                let key = &format!("thread_{i}_key_{j}");
                assert_eq!(
                    mmkv.read()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .get(key)
                        .unwrap()
                        .decode_i32()
                        .unwrap(),
                    j
                )
            }
        }
        mmkv.write().unwrap().as_mut().unwrap().clear_data();
        assert!(!Path::new(file).exists());
    }
}
