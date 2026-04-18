use crate::Error::InstanceClosed;
use crate::core::buffer::{Buffer, Decoder, FromBytes, ProvideTypeToken};
use crate::core::config::Config;
#[cfg(not(feature = "encryption"))]
use crate::core::crc::CrcEncoder;
#[cfg(feature = "encryption")]
use crate::core::encrypt::Encryptor;
use crate::core::io_looper::IOLooper;
use crate::core::memory_map::{MemoryMap, MmapHandle};
use crate::core::shared_state::{SharedKvMap, SharedState};
use crate::core::writer::IOWriter;
use crate::{Error, Result};
#[cfg(feature = "encryption")]
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

const LOG_TAG: &str = "MMKV:Core";

pub struct MmkvImpl {
    is_valid: bool,
    io_looper: IOLooper<IOWriter>,
    shared_kv: SharedKvMap,
    next_seq: Arc<AtomicU64>,
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
        let encoder = Box::new(CrcEncoder);
        let mm = MemoryMap::new(&config.file, config.file_size()?)?;
        #[cfg(feature = "encryption")]
        {
            let write_offset = mm.write_offset()?;
            if write_offset > mm.content_start_offset() {
                let bytes = mm.read(mm.content_start_offset()..write_offset)?;
                encryptor.recover_current_nonce(bytes)?;
            }
        }
        #[cfg(feature = "encryption")]
        let decoder = Box::new(encryptor.clone());
        #[cfg(not(feature = "encryption"))]
        let decoder = Box::new(CrcEncoder);
        let mmap_base = mm.base_ptr();
        let (kv_map, decoded_position) = mm
            .iter(|bytes, position| decoder.decode_bytes(bytes, position))?
            .into_map(mmap_base);
        let item_count = kv_map.len();
        let content_len = mm.write_offset()?;
        let file_size = mm.len();
        let mmap_handle = mm.to_handle();
        let shared_kv = SharedState::new(mmap_handle, kv_map);
        let next_seq = Arc::new(AtomicU64::new(1));
        let io_writer = IOWriter::new(
            config,
            mm,
            decoded_position,
            Arc::clone(&shared_kv),
            encoder,
            #[cfg(feature = "encryption")]
            encryptor.clone(),
        );
        let mmkv = MmkvImpl {
            is_valid: true,
            io_looper: IOLooper::new(io_writer),
            shared_kv,
            next_seq,
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
        debug_assert!(matches!(&raw_buffer, Buffer::Owned { kv, .. } if kv.key == key));
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let raw_buffer = raw_buffer.with_seq(seq);
        let previous = {
            let mut kv_map = self
                .shared_kv
                .kv_map
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
                .kv_map
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

    pub fn get<T: ProvideTypeToken + FromBytes>(&self, key: &str) -> Result<T> {
        if !self.is_valid {
            return Err(InstanceClosed);
        }
        // Hold kv_map.read() across parse so the writer cannot swap the mmap
        // (via kv_map.write() inside shadow-file trim) between when we look up
        // the Slice offsets and when we dereference those offsets in the mmap.
        let kv_guard = self
            .shared_kv
            .kv_map
            .read()
            .map_err(|e| Error::LockError(e.to_string()))?;
        let mmap_guard = self.shared_kv.mmap.load();
        let mmap: &MmapHandle = &mmap_guard;
        match kv_guard.get(key) {
            Some(buf) => {
                #[cfg(not(feature = "encryption"))]
                {
                    buf.parse::<T>(mmap)
                }
                #[cfg(feature = "encryption")]
                {
                    self.parse_buffer::<T>(buf, mmap)
                }
            }
            None => Err(Error::KeyNotFound),
        }
    }

    #[cfg(feature = "encryption")]
    fn parse_buffer<T: ProvideTypeToken + FromBytes>(
        &self,
        buf: &Buffer,
        mmap: &MmapHandle,
    ) -> Result<T> {
        match buf {
            Buffer::Owned { .. } => buf.parse::<T>(mmap),
            Buffer::Slice(loc) => {
                // Decrypt record bytes from mmap, then parse via Owned path.
                let ciphertext = mmap.read(loc.byte_range());
                let kv_bytes = self.encryptor.decrypt_current(ciphertext, loc.position)?;
                let owned = Buffer::from_encoded_bytes(&kv_bytes)?;
                owned.parse::<T>(mmap)
            }
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
                .kv_map
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
                .kv_map
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
                .kv_map
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
    use std::io::{Seek, SeekFrom, Write};
    use std::mem::size_of;
    use std::path::Path;
    use std::sync::RwLock;
    use std::{fs, thread};

    use crate::Error::{IOError, KeyNotFound};
    use crate::LogLevel::Debug;
    use crate::MMKV;
    use crate::core::buffer::Buffer;
    use crate::core::config::Config;
    #[cfg(feature = "encryption")]
    use crate::core::encrypt::Encryptor;
    use crate::core::memory_map::MemoryMap;
    use crate::core::mmkv_impl::MmkvImpl;

    #[cfg(feature = "encryption")]
    const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

    fn init(config: &Config) -> MmkvImpl {
        MMKV::set_log_level(Debug);
        MmkvImpl::new(
            Config::new(&config.path, config.page_size).unwrap(),
            #[cfg(feature = "encryption")]
            TEST_KEY,
        )
        .unwrap()
    }

    fn write_offset_at(path: &str) -> usize {
        use std::fs::OpenOptions;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .unwrap();
        let len = file.metadata().unwrap().len();
        MemoryMap::new(&file, len).unwrap().write_offset().unwrap()
    }

    #[test]
    #[cfg(not(feature = "encryption"))]
    fn test_trim_and_expand_default() {
        let file_path = "test_trim_and_expand_default";
        let _ = fs::remove_file(file_path);
        assert!(!Path::new(file_path).exists());
        let _ = fs::remove_file(format!("{}.meta", file_path));
        let config = &Config::new(Path::new(file_path), 100).unwrap();
        let mut mmkv = init(config);
        mmkv.put("key1", Buffer::new("key1", 1)).unwrap(); // + 17
        assert_eq!(mmkv.get::<i32>("key1"), Ok(1));
        drop(mmkv);
        assert_eq!(write_offset_at(file_path), 25);

        mmkv = init(config);
        mmkv.put("key2", Buffer::new("key2", 2)).unwrap(); // + 17
        mmkv.put("key3", Buffer::new("key3", 3)).unwrap(); // + 17
        mmkv.put("key1", Buffer::new("key1", 4)).unwrap(); // + 17
        mmkv.put("key2", Buffer::new("key2", 5)).unwrap(); // + 17
        drop(mmkv);
        assert_eq!(write_offset_at(file_path), 93);

        mmkv = init(config);
        mmkv.put("key1", Buffer::new("key1", 6)).unwrap(); // + 17, trim, 3 items remain
        drop(mmkv);
        assert_eq!(write_offset_at(file_path), 59);

        mmkv = init(config);
        assert_eq!(mmkv.get::<i32>("key1"), Ok(6));
        assert_eq!(mmkv.get::<i32>("key2"), Ok(5));
        mmkv.put("key4", Buffer::new("key4", 4)).unwrap();
        mmkv.put("key5", Buffer::new("key5", 5)).unwrap(); // 93
        mmkv.put("key6", Buffer::new("key6", 6)).unwrap(); // expand, 110
        drop(mmkv);
        assert_eq!(write_offset_at(file_path), 110);
        assert_eq!(fs::metadata(file_path).unwrap().len(), 200);

        mmkv = init(config);
        mmkv.put("key7", Buffer::new("key7", 7)).unwrap();
        drop(mmkv);
        assert_eq!(write_offset_at(file_path), 127);

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
        let mut mmkv = init(config);
        mmkv.put("key1", Buffer::new("key1", 1)).unwrap(); // + 24
        assert_eq!(mmkv.get::<i32>("key1"), Ok(1));
        drop(mmkv);
        assert_eq!(write_offset_at(file), 32);

        mmkv = init(config);
        mmkv.put("key2", Buffer::new("key2", 2)).unwrap(); // + 24
        mmkv.put("key3", Buffer::new("key3", 3)).unwrap(); // + 24
        drop(mmkv);
        assert_eq!(write_offset_at(file), 80);

        mmkv = init(config);
        mmkv.put("key1", Buffer::new("key1", 4)).unwrap(); // + 24 trim
        mmkv.put("key2", Buffer::new("key2", 5)).unwrap(); // + 24 trim
        drop(mmkv);
        assert_eq!(write_offset_at(file), 80);

        mmkv = init(config);
        assert_eq!(mmkv.get::<i32>("key1"), Ok(4));
        assert_eq!(mmkv.get::<i32>("key2"), Ok(5));
        mmkv.put("key4", Buffer::new("key4", 4)).unwrap(); // + 24
        drop(mmkv);
        assert_eq!(write_offset_at(file), 104);
        assert_eq!(fs::metadata(file).unwrap().len(), 200);

        mmkv = init(config);
        mmkv.put("key5", Buffer::new("key5", 5)).unwrap(); // + 24
        drop(mmkv);
        assert_eq!(write_offset_at(file), 128);

        mmkv = init(config);
        mmkv.clear_data().unwrap();
        assert!(!Path::new(file).exists());
    }

    #[test]
    #[cfg(feature = "encryption")]
    fn test_reopen_recovers_previous_nonce_after_interrupted_rotation() {
        let file = "test_recover_previous_nonce";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{file}.meta"));
        let config = Config::new(Path::new(file), 128).unwrap();
        let mut mmkv = init(&config);
        mmkv.put("key1", Buffer::new("key1", 7)).unwrap();
        drop(mmkv);

        let encryptor = Encryptor::init(Path::new(file), TEST_KEY);
        encryptor.rotate_nonce().unwrap();
        drop(encryptor);

        let mut mmkv = init(&config);
        assert_eq!(mmkv.get::<i32>("key1"), Ok(7));
        mmkv.put("key2", Buffer::new("key2", 8)).unwrap();
        drop(mmkv);

        let mut mmkv = init(&config);
        assert_eq!(mmkv.get::<i32>("key1"), Ok(7));
        assert_eq!(mmkv.get::<i32>("key2"), Ok(8));
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
                assert_eq!(mmkv.get::<i32>(key).unwrap(), j)
            }
        }
        assert_eq!(
            mmkv.get::<i32>("test_multi_thread_mmkv_repeat_key"),
            Err(KeyNotFound)
        );
        mmkv.clear_data().unwrap();
        assert!(!Path::new(file).exists());
    }

    // Regression test for the reader-vs-trim race:
    // Before the fix, a get() that dropped kv_map.read() before parse could read
    // torn bytes from the live mmap while the IO thread reset it for a shadow-file trim.
    #[test]
    fn test_concurrent_reads_during_trim() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let file = "test_concurrent_reads_during_trim";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{file}.meta"));
        let config = &Config::new(Path::new(file), 96).unwrap();

        let mut mmkv = init(config);
        mmkv.put("stable", Buffer::new("stable", 42i32)).unwrap();
        drop(mmkv);

        let mmkv = Arc::new(RwLock::new(init(config)));
        let errors = Arc::new(AtomicUsize::new(0));
        let iters = 600;

        thread::scope(|s| {
            for _ in 0..4 {
                let mmkv = Arc::clone(&mmkv);
                let errors = Arc::clone(&errors);
                s.spawn(move || {
                    for _ in 0..iters {
                        match mmkv.read().unwrap().get::<i32>("stable") {
                            Ok(42) => {}
                            _ => {
                                errors.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                });
            }
            // Writer triggers frequent trims by re-putting a large value repeatedly.
            {
                let mmkv = Arc::clone(&mmkv);
                s.spawn(move || {
                    let pad = vec![7u8; 64];
                    for _ in 0..iters {
                        let _ = mmkv
                            .write()
                            .unwrap()
                            .put("trim_trigger", Buffer::new("trim_trigger", pad.as_slice()));
                    }
                });
            }
        });

        assert_eq!(
            errors.load(Ordering::Relaxed),
            0,
            "concurrent reads during trim observed wrong values"
        );

        // Drop before cleanup so the IO thread finishes any queued trim (which could
        // rename a .tmp file back to the original path after we delete it).
        drop(mmkv);

        // Stable value must survive all trim cycles.
        assert_eq!(init(config).get::<i32>("stable"), Ok(42));
        init(config).clear_data().unwrap();
        let _ = fs::remove_file(format!("{file}.meta"));
    }

    #[test]
    fn test_sync_visibility_for_put_and_delete() {
        let file = "test_sync_visibility_for_put_and_delete";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{}.meta", file));
        let config = &Config::new(Path::new(file), 128).unwrap();
        let mut mmkv = init(config);

        mmkv.put("sync_key", Buffer::new("sync_key", 7)).unwrap();
        assert_eq!(mmkv.get::<i32>("sync_key"), Ok(7));

        mmkv.delete("sync_key").unwrap();
        assert_eq!(mmkv.get::<i32>("sync_key"), Err(KeyNotFound));

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
        assert_eq!(mmkv.get::<i32>("rollback_key"), Err(KeyNotFound));

        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{}.meta", file));
    }

    #[test]
    fn test_init_rejects_invalid_mmap_header() {
        let file = "test_invalid_mmap_header";
        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{}.meta", file));
        let config = Config::new(Path::new(file), (size_of::<u64>() + 1) as u64).unwrap();
        let mut file_handle = config.file.try_clone().unwrap();
        file_handle.seek(SeekFrom::Start(0)).unwrap();
        file_handle.write_all(&2u64.to_be_bytes()).unwrap();
        file_handle.sync_all().unwrap();

        let result = MmkvImpl::new(
            config.try_clone().unwrap(),
            #[cfg(feature = "encryption")]
            TEST_KEY,
        );
        assert_eq!(
            result.err(),
            Some(IOError("invalid mmap content length 2, max 1".to_string()))
        );

        let _ = fs::remove_file(file);
        let _ = fs::remove_file(format!("{}.meta", file));
    }
}
