use crate::Error::InstanceClosed;
use crate::core::buffer::{Buffer, Decoder};
use crate::core::config::Config;
#[cfg(not(feature = "encryption"))]
use crate::core::crc::CrcEncoderDecoder;
#[cfg(feature = "encryption")]
use crate::core::encrypt::Encryptor;
use crate::core::io_looper::IOLooper;
use crate::core::memory_map::MemoryMap;
use crate::core::shared_state::{SharedKvMap, new_shared_kv_map};
use crate::core::writer::IOWriter;
use crate::{Error, Result};
#[cfg(feature = "encryption")]
use std::fs;
use std::sync::Arc;
use std::time::Instant;

const LOG_TAG: &str = "MMKV:Core";

pub struct MmkvImpl {
    is_valid: bool,
    io_looper: IOLooper<IOWriter>,
    shared_kv: SharedKvMap,
    #[cfg(feature = "encryption")]
    encryptor: Encryptor,
}

impl MmkvImpl {
    pub fn new(config: Config, #[cfg(feature = "encryption")] key: &str) -> Result<Self> {
        let time_start = Instant::now();
        #[cfg(feature = "encryption")]
        let encryptor = Encryptor::init(&config.path, key);
        #[cfg(feature = "encryption")]
        let encoder = Box::new(encryptor.clone());
        #[cfg(not(feature = "encryption"))]
        let encoder = Box::new(CrcEncoderDecoder);
        let mm = MemoryMap::new(&config.file, config.file_size()? as usize)?;
        #[cfg(feature = "encryption")]
        let decoder = Box::new(encryptor.clone());
        #[cfg(not(feature = "encryption"))]
        let decoder = Box::new(CrcEncoderDecoder);
        let (kv_map, decoded_position) = mm
            .iter(|bytes, position| decoder.decode_bytes(bytes, position))
            .into_map();
        let item_count = kv_map.len();
        let content_len = mm.write_offset();
        let file_size = mm.len();
        let shared_kv = new_shared_kv_map(kv_map);
        let io_writer = IOWriter::new(
            config,
            mm,
            decoded_position,
            Arc::clone(&shared_kv),
            encoder,
        );
        let mmkv = MmkvImpl {
            is_valid: true,
            io_looper: IOLooper::new(io_writer),
            shared_kv,
            #[cfg(feature = "encryption")]
            encryptor,
        };
        info!(
            LOG_TAG,
            "instance initialized, read {} items, content len {}, file size {}, cost {:?}",
            item_count,
            content_len,
            file_size,
            time_start.elapsed()
        );
        Ok(mmkv)
    }

    pub fn put(&mut self, key: &str, raw_buffer: Buffer) -> Result<()> {
        if !self.is_valid {
            return Err(InstanceClosed);
        }
        debug_assert_eq!(key, raw_buffer.key());
        let previous = {
            let mut kv_map = self
                .shared_kv
                .write()
                .map_err(|e| Error::LockError(e.to_string()))?;
            kv_map.insert(key.to_string(), raw_buffer.clone())
        };
        let duplicated = previous.is_some();
        if let Err(err) = self
            .io_looper
            .post(move |writer| writer.write(raw_buffer, duplicated))
        {
            let mut kv_map = self
                .shared_kv
                .write()
                .map_err(|e| Error::LockError(e.to_string()))?;
            if let Some(buffer) = previous {
                kv_map.insert(key.to_string(), buffer);
            } else {
                kv_map.remove(key);
            }
            return Err(err);
        }
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Buffer> {
        if !self.is_valid {
            return Err(InstanceClosed);
        }
        match self
            .shared_kv
            .read()
            .map_err(|e| Error::LockError(e.to_string()))?
            .get(key)
            .cloned()
        {
            Some(buffer) => Ok(buffer),
            None => Err(Error::KeyNotFound),
        }
    }

    pub fn delete(&mut self, key: &str) -> Result<()> {
        if !self.is_valid {
            return Err(InstanceClosed);
        }
        let key = key.to_string();
        let previous = {
            let mut kv_map = self
                .shared_kv
                .write()
                .map_err(|e| Error::LockError(e.to_string()))?;
            kv_map.remove(&key)
        };
        if previous.is_none() {
            return Ok(());
        }
        if let Err(err) = self.io_looper.post({
            let key = key.clone();
            move |writer| writer.write(Buffer::deleted_buffer(&key), true)
        }) {
            let mut kv_map = self
                .shared_kv
                .write()
                .map_err(|e| Error::LockError(e.to_string()))?;
            kv_map.insert(key, previous.unwrap());
            return Err(err);
        }
        Ok(())
    }

    pub fn clear_data(&mut self) -> Result<()> {
        if !self.is_valid {
            warn!(LOG_TAG, "instance already closed");
            return Ok(());
        }
        self.is_valid = false;
        #[cfg(feature = "encryption")]
        let meta_file = self.encryptor.meta_file_path.clone();
        let shared_kv = Arc::clone(&self.shared_kv);
        self.io_looper.call(move |writer| {
            writer.remove_file()?;
            shared_kv
                .write()
                .map_err(|e| Error::LockError(e.to_string()))?
                .clear();
            #[cfg(feature = "encryption")]
            let _ = fs::remove_file(meta_file);
            info!(LOG_TAG, "data cleared");
            Ok(())
        })?;
        self.io_looper.quit()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::RwLock;
    use std::{fs, thread};

    use crate::Error::KeyNotFound;
    use crate::LogLevel::Debug;
    use crate::MMKV;
    use crate::core::buffer::Buffer;
    use crate::core::config::Config;
    use crate::core::memory_map::MemoryMap;
    use crate::core::mmkv_impl::MmkvImpl;

    #[cfg(feature = "encryption")]
    const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

    fn init(config: &Config) -> MmkvImpl {
        MMKV::set_log_level(Debug);
        MmkvImpl::new(
            config.try_clone().unwrap(),
            #[cfg(feature = "encryption")]
            TEST_KEY,
        )
        .unwrap()
    }

    #[test]
    #[cfg(not(feature = "encryption"))]
    fn test_trim_and_expand_default() {
        let file_path = "test_trim_and_expand_default";
        let _ = fs::remove_file(file_path);
        assert!(!Path::new(file_path).exists());
        let _ = fs::remove_file(format!("{}.meta", file_path));
        let config = &Config::new(Path::new(file_path), 100).unwrap();
        let mm = MemoryMap::new(&config.file, 200).unwrap();
        let mut mmkv = init(config);
        mmkv.put("key1", Buffer::new("key1", 1)).unwrap(); // + 17
        assert_eq!(mmkv.get("key1").unwrap().parse::<i32>(), Ok(1));
        drop(mmkv);
        assert_eq!(mm.write_offset(), 25);

        mmkv = init(config);
        mmkv.put("key2", Buffer::new("key2", 2)).unwrap(); // + 17
        mmkv.put("key3", Buffer::new("key3", 3)).unwrap(); // + 17
        mmkv.put("key1", Buffer::new("key1", 4)).unwrap(); // + 17
        mmkv.put("key2", Buffer::new("key2", 5)).unwrap(); // + 17
        drop(mmkv);
        assert_eq!(mm.write_offset(), 93);

        mmkv = init(config);
        mmkv.put("key1", Buffer::new("key1", 6)).unwrap(); // + 17, trim, 3 items remain
        drop(mmkv);
        assert_eq!(mm.write_offset(), 59);

        mmkv = init(config);
        assert_eq!(mmkv.get("key1").unwrap().parse::<i32>(), Ok(6));
        assert_eq!(mmkv.get("key2").unwrap().parse::<i32>(), Ok(5));
        mmkv.put("key4", Buffer::new("key4", 4)).unwrap();
        mmkv.put("key5", Buffer::new("key5", 5)).unwrap(); // 93
        mmkv.put("key6", Buffer::new("key6", 6)).unwrap(); // expand, 110
        drop(mmkv);
        assert_eq!(mm.write_offset(), 110);
        assert_eq!(config.file_size().unwrap(), 200);

        mmkv = init(config);
        mmkv.put("key7", Buffer::new("key7", 7)).unwrap();
        drop(mmkv);
        assert_eq!(mm.write_offset(), 127);

        mmkv = init(config);
        mmkv.clear_data().unwrap();
        assert!(!Path::new(file_path).exists());
    }

    #[test]
    #[cfg(feature = "encryption")]
    fn test_trim_and_expand_encrypt() {
        let file = "test_trim_and_expand_encrypt";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{file}.meta"));
        let config = &Config::new(Path::new(file), 100).unwrap();
        let mm = MemoryMap::new(&config.file, 200).unwrap();
        let mut mmkv = init(config);
        mmkv.put("key1", Buffer::new("key1", 1)).unwrap(); // + 24
        assert_eq!(mmkv.get("key1").unwrap().parse::<i32>(), Ok(1));
        drop(mmkv);
        assert_eq!(mm.write_offset(), 32);

        mmkv = init(config);
        mmkv.put("key2", Buffer::new("key2", 2)).unwrap(); // + 24
        mmkv.put("key3", Buffer::new("key3", 3)).unwrap(); // + 24
        drop(mmkv);
        assert_eq!(mm.write_offset(), 80);

        mmkv = init(config);
        mmkv.put("key1", Buffer::new("key1", 4)).unwrap(); // + 24 trim
        mmkv.put("key2", Buffer::new("key2", 5)).unwrap(); // + 24 trim
        drop(mmkv);
        assert_eq!(mm.write_offset(), 80);

        mmkv = init(config);
        assert_eq!(mmkv.get("key1").unwrap().parse::<i32>(), Ok(4));
        assert_eq!(mmkv.get("key2").unwrap().parse::<i32>(), Ok(5));
        mmkv.put("key4", Buffer::new("key4", 4)).unwrap(); // + 24
        drop(mmkv);
        assert_eq!(mm.write_offset(), 104);
        assert_eq!(config.file_size().unwrap(), 200);

        mmkv = init(config);
        mmkv.put("key5", Buffer::new("key5", 5)).unwrap(); // + 24
        drop(mmkv);
        assert_eq!(mm.write_offset(), 128);

        mmkv = init(config);
        mmkv.clear_data().unwrap();
        assert!(!Path::new(file).exists());
    }

    #[test]
    fn test_multi_thread_mmkv() {
        let file = "test_multi_thread_mmkv";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{}.meta", file));
        let config = &Config::new(Path::new(file), 4096).unwrap();
        let mmkv = RwLock::new(Some(init(config)));
        let loop_count = 1000;
        let action = |thread_id: &str| {
            for i in 0..loop_count {
                let key = &format!("{thread_id}_key_{i}");
                mmkv.write()
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .put(key, Buffer::new(key, i))
                    .unwrap();
            }
        };
        thread::scope(|s| {
            s.spawn(|| {
                let repeat_key = "test_multi_thread_mmkv_repeat_key";
                for i in 0..loop_count {
                    let mut lock = mmkv.write().unwrap();
                    let mmkv = lock.as_mut().unwrap();
                    if i % 2 == 0 {
                        mmkv.put(repeat_key, Buffer::new(repeat_key, i)).unwrap();
                    } else {
                        mmkv.delete(repeat_key).unwrap();
                    }
                }
            });
            for i in 0..2 {
                s.spawn(move || action(format!("thread_{i}").as_ref()));
            }
        });
        drop(mmkv.write().unwrap().take());
        let mut mmkv = init(config);
        for i in 0..2 {
            for j in 0..loop_count {
                let key = &format!("thread_{i}_key_{j}");
                assert_eq!(mmkv.get(key).unwrap().parse::<i32>().unwrap(), j)
            }
        }
        assert_eq!(
            mmkv.get("test_multi_thread_mmkv_repeat_key"),
            Err(KeyNotFound)
        );
        mmkv.clear_data().unwrap();
        assert!(!Path::new(file).exists());
    }

    #[test]
    fn test_sync_visibility_for_put_and_delete() {
        let file = "test_sync_visibility_for_put_and_delete";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{}.meta", file));
        let config = &Config::new(Path::new(file), 128).unwrap();
        let mut mmkv = init(config);

        mmkv.put("sync_key", Buffer::new("sync_key", 7)).unwrap();
        assert_eq!(mmkv.get("sync_key").unwrap().parse::<i32>(), Ok(7));

        mmkv.delete("sync_key").unwrap();
        assert_eq!(mmkv.get("sync_key"), Err(KeyNotFound));

        mmkv.clear_data().unwrap();
        assert!(!Path::new(file).exists());
    }

    #[test]
    fn test_post_failure_rolls_back_shared_state() {
        let file = "test_post_failure_rolls_back_shared_state";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{}.meta", file));
        let config = &Config::new(Path::new(file), 128).unwrap();
        let mut mmkv = init(config);

        mmkv.io_looper.quit().unwrap();
        assert!(
            mmkv.put("rollback_key", Buffer::new("rollback_key", 1))
                .is_err()
        );
        assert_eq!(mmkv.get("rollback_key"), Err(KeyNotFound));

        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{}.meta", file));
    }
}
