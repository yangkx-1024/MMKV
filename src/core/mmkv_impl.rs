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
        let encryptor = Encryptor::init(&config.path, key)?;
        #[cfg(feature = "encryption")]
        let encoder = Box::new(encryptor.clone());
        #[cfg(not(feature = "encryption"))]
        let encoder = Box::new(CrcEncoder);
        let mut mm = MemoryMap::new(&config.file, config.file_size()?)?;
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
        let (kv_map, decoded_position, decoded_end) = mm
            .iter(|bytes, position| decoder.decode_bytes(bytes, position))?
            .into_map(mmap_base);
        let write_offset = mm.write_offset()?;
        if decoded_end < write_offset {
            // The tail cannot be framed, so every later append would land after garbage
            // and every reopen would stop here again. Drop it and keep the good prefix.
            error!(
                LOG_TAG,
                "discarding {} undecodable bytes at offset {}, moving write offset {} -> {}",
                write_offset - decoded_end,
                decoded_end,
                write_offset,
                decoded_end
            );
            mm.truncate_content(decoded_end)?;
        }
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
        // Run the write on the IO thread and wait for it, so that `Ok(())` means the
        // record is in the memory-mapped file. On failure restore the previous entry
        // so readers never see a value that never reached disk.
        if let Err(err) = self
            .io_looper
            .call(move |writer| writer.write(raw_buffer, duplicated))
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
        // Same contract as `put`: wait for the tombstone to be written, and restore the
        // entry on failure so the key cannot silently resurrect on the next launch.
        if let Err(err) = self.io_looper.call({
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

/// Unit tests for the parts of `MmkvImpl` that need private access: the exact byte
/// offsets of trim and expand, rollback of a failed write, recovery from a damaged file
/// and the closed-instance contract. Behaviour that is observable through `MMKV` is
/// tested in `tests/` instead.
#[cfg(test)]
mod tests {
    use std::fs;

    use crate::Error::{DataInvalid, IOError, InstanceClosed, KeyNotFound};
    use crate::core::buffer::{Buffer, ProvideTypeToken};
    use crate::core::config::Config;
    #[cfg(feature = "encryption")]
    use crate::core::encrypt::Encryptor;
    use crate::core::test_support::{
        self, HEADER_LEN, page_for, record_len, temp_config, write_offset_at,
    };

    use tempfile::tempdir;

    /// One feature-neutral walk through append -> trim -> expand, checking the write
    /// offset after every step. Record sizes are measured with the codec of the current
    /// build flavour (17 bytes with CRC framing, 24 with AEAD framing) and the page is
    /// sized from that measurement, so the same script holds for both.
    #[test]
    fn trim_and_expand_keep_the_file_at_the_expected_offsets() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mmkv");
        // Every key is 4 chars and every value an i32, so all records are the same size.
        let rec = record_len(&path, "key1", 1i32);
        let page = page_for(5, rec);
        let config = Config::new(&path, page).unwrap();
        let config = &config;
        let offset_of = |records: usize| page_for(records, rec) as usize;

        let mut mmkv = test_support::open(config);
        mmkv.put("key1", Buffer::new("key1", 1)).unwrap();
        assert_eq!(mmkv.get::<i32>("key1"), Ok(1));
        drop(mmkv);
        assert_eq!(write_offset_at(&path), offset_of(1));

        // Fill the page exactly: two new keys plus two duplicates.
        mmkv = test_support::open(config);
        mmkv.put("key2", Buffer::new("key2", 2)).unwrap();
        mmkv.put("key3", Buffer::new("key3", 3)).unwrap();
        mmkv.put("key1", Buffer::new("key1", 4)).unwrap();
        mmkv.put("key2", Buffer::new("key2", 5)).unwrap();
        drop(mmkv);
        assert_eq!(write_offset_at(&path), offset_of(5));

        // The page is full and the put is a duplicate, so the writer trims instead of
        // expanding: only the three live keys survive.
        mmkv = test_support::open(config);
        mmkv.put("key1", Buffer::new("key1", 6)).unwrap();
        drop(mmkv);
        assert_eq!(write_offset_at(&path), offset_of(3));
        assert_eq!(fs::metadata(&path).unwrap().len(), page);

        mmkv = test_support::open(config);
        assert_eq!(mmkv.get::<i32>("key1"), Ok(6));
        assert_eq!(mmkv.get::<i32>("key2"), Ok(5));
        assert_eq!(mmkv.get::<i32>("key3"), Ok(3));
        mmkv.put("key4", Buffer::new("key4", 4)).unwrap();
        mmkv.put("key5", Buffer::new("key5", 5)).unwrap();
        assert_eq!(mmkv.get::<i32>("key5"), Ok(5));
        // Nothing is pending a trim, so a sixth key doubles the file instead.
        mmkv.put("key6", Buffer::new("key6", 6)).unwrap();
        drop(mmkv);
        assert_eq!(write_offset_at(&path), offset_of(6));
        assert_eq!(fs::metadata(&path).unwrap().len(), page * 2);

        mmkv = test_support::open(config);
        assert_eq!(mmkv.get::<i32>("key6"), Ok(6));
        mmkv.put("key7", Buffer::new("key7", 7)).unwrap();
        drop(mmkv);
        assert_eq!(write_offset_at(&path), offset_of(7));

        mmkv = test_support::open(config);
        for (key, value) in [
            ("key1", 6),
            ("key2", 5),
            ("key3", 3),
            ("key4", 4),
            ("key5", 5),
            ("key6", 6),
            ("key7", 7),
        ] {
            assert_eq!(mmkv.get::<i32>(key), Ok(value), "after reopen: {key}");
        }
        mmkv.clear_data().unwrap();
        assert!(!path.exists());
    }

    #[test]
    #[cfg(feature = "encryption")]
    fn reopen_recovers_the_previous_nonce_after_an_interrupted_rotation() {
        let (_dir, config) = temp_config(128);
        let mut mmkv = test_support::open(&config);
        mmkv.put("key1", Buffer::new("key1", 7)).unwrap();
        drop(mmkv);

        let encryptor = Encryptor::init(&config.path, test_support::TEST_KEY).unwrap();
        encryptor.rotate_nonce().unwrap();
        drop(encryptor);

        let mut mmkv = test_support::open(&config);
        assert_eq!(mmkv.get::<i32>("key1"), Ok(7));
        mmkv.put("key2", Buffer::new("key2", 8)).unwrap();
        drop(mmkv);

        let mut mmkv = test_support::open(&config);
        assert_eq!(mmkv.get::<i32>("key1"), Ok(7));
        assert_eq!(mmkv.get::<i32>("key2"), Ok(8));
        mmkv.clear_data().unwrap();
        assert!(!config.path.exists());
    }

    #[test]
    fn a_closed_looper_rolls_back_the_shared_state() {
        let (_dir, config) = temp_config(128);
        let mut mmkv = test_support::open(&config);

        mmkv.io_looper.quit().unwrap();
        assert!(
            mmkv.put("rollback_key", Buffer::new("rollback_key", 1))
                .is_err()
        );
        assert_eq!(mmkv.get::<i32>("rollback_key"), Err(KeyNotFound));
    }

    /// A failed write must be reported by `put`/`delete`, must leave the in-memory map
    /// equal to what is on disk, and must not wedge the instance.
    #[test]
    fn a_failed_write_returns_err_and_rolls_back() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mmkv");

        let v_k = vec![1u8; 60];
        let v_j = vec![2u8; 30];
        // Size the file so that exactly these two records fit; any further write (a
        // duplicate put or a tombstone) then has to go through a shadow-file trim.
        let page = HEADER_LEN
            + record_len(&path, "k", v_k.as_slice())
            + record_len(&path, "j", v_j.as_slice());
        let config = Config::new(&path, page as u64).unwrap();
        let mut mmkv = test_support::open(&config);
        mmkv.put("k", Buffer::new("k", v_k.as_slice())).unwrap();
        mmkv.put("j", Buffer::new("j", v_j.as_slice())).unwrap();

        // Occupy the tmp paths the next two trims will `create_new`, so both trims fail.
        let blockers = test_support::block_next_trims(&config, 2);

        let new_k = vec![3u8; 60];
        assert!(mmkv.put("k", Buffer::new("k", new_k.as_slice())).is_err());
        assert_eq!(mmkv.get::<Vec<u8>>("k"), Ok(v_k.clone()));
        assert!(mmkv.delete("j").is_err());
        assert_eq!(mmkv.get::<Vec<u8>>("j"), Ok(v_j.clone()));
        drop(mmkv);
        for blocker in blockers {
            fs::remove_file(blocker).unwrap();
        }

        // Disk agrees with what the callers were told.
        let mut mmkv = test_support::open(&config);
        assert_eq!(mmkv.get::<Vec<u8>>("k"), Ok(v_k));
        assert_eq!(mmkv.get::<Vec<u8>>("j"), Ok(v_j));
        // And once the fault is gone the same operations succeed.
        mmkv.put("k", Buffer::new("k", new_k.as_slice())).unwrap();
        mmkv.delete("j").unwrap();
        assert_eq!(mmkv.get::<Vec<u8>>("k"), Ok(new_k.clone()));
        assert_eq!(mmkv.get::<Vec<u8>>("j"), Err(KeyNotFound));
        drop(mmkv);

        let mut mmkv = test_support::open(&config);
        assert_eq!(mmkv.get::<Vec<u8>>("k"), Ok(new_k));
        assert_eq!(mmkv.get::<Vec<u8>>("j"), Err(KeyNotFound));
        mmkv.clear_data().unwrap();
        assert!(!path.exists());
    }

    /// A file whose tail cannot be framed must open, keep every record before the
    /// damage, drop the damaged bytes, and stay writable across reopen.
    #[test]
    fn init_recovers_from_a_corrupted_tail() {
        let (_dir, config) = temp_config(256);
        let file = &config.path;
        let mut mmkv = test_support::open(&config);
        mmkv.put("k1", Buffer::new("k1", 1)).unwrap();
        drop(mmkv);
        let offset_after_k1 = write_offset_at(file);
        let mut mmkv = test_support::open(&config);
        mmkv.put("k2", Buffer::new("k2", 2)).unwrap();
        drop(mmkv);
        let good_end = write_offset_at(file);
        let rec = good_end - offset_after_k1;

        // Append a frame whose declared length runs far past the content, with the header
        // bumped so the store believes those bytes are live records.
        let garbage = [0xFF, 0xFF, 0xFF, 0xFF, 0xAA, 0xBB];
        test_support::append_raw(file, &garbage);
        assert_eq!(write_offset_at(file), good_end + garbage.len());

        let mut mmkv = test_support::open(&config);
        assert_eq!(mmkv.get::<i32>("k1"), Ok(1));
        assert_eq!(mmkv.get::<i32>("k2"), Ok(2));
        mmkv.put("k3", Buffer::new("k3", 3)).unwrap();
        drop(mmkv);
        // The garbage was discarded and k3 landed right after the last good record.
        assert_eq!(write_offset_at(file), good_end + rec);

        let mut mmkv = test_support::open(&config);
        assert_eq!(mmkv.get::<i32>("k1"), Ok(1));
        assert_eq!(mmkv.get::<i32>("k2"), Ok(2));
        assert_eq!(mmkv.get::<i32>("k3"), Ok(3));
        mmkv.clear_data().unwrap();
        assert!(!file.exists());
    }

    /// A record whose value is shorter than its type needs must yield `DataInvalid`
    /// from `get`, before and after a reopen, instead of panicking.
    #[test]
    fn get_reports_data_invalid_for_a_value_that_is_too_short() {
        let (_dir, config) = temp_config(256);
        let mut mmkv = test_support::open(&config);
        let i32_token = <i32 as ProvideTypeToken>::type_token().token;
        let bool_token = <bool as ProvideTypeToken>::type_token().token;
        mmkv.put(
            "short_i32",
            Buffer::from_kv("short_i32", i32_token, vec![1, 2]),
        )
        .unwrap();
        mmkv.put(
            "empty_bool",
            Buffer::from_kv("empty_bool", bool_token, vec![]),
        )
        .unwrap();
        assert_eq!(mmkv.get::<i32>("short_i32"), Err(DataInvalid));
        assert_eq!(mmkv.get::<bool>("empty_bool"), Err(DataInvalid));
        drop(mmkv);

        let mut mmkv = test_support::open(&config);
        assert_eq!(mmkv.get::<i32>("short_i32"), Err(DataInvalid));
        assert_eq!(mmkv.get::<bool>("empty_bool"), Err(DataInvalid));
        mmkv.clear_data().unwrap();
        assert!(!config.path.exists());
    }

    #[test]
    fn init_rejects_a_header_that_claims_more_content_than_the_file_holds() {
        let (_dir, config) = temp_config((HEADER_LEN + 1) as u64);
        test_support::set_content_len(&config.path, 2);

        assert_eq!(
            test_support::try_open(&config).err(),
            Some(IOError("invalid mmap content length 2, max 1".to_string()))
        );
    }

    /// `clear_data` closes the instance for good; every later operation must say so
    /// rather than silently writing to a store that is no longer there.
    #[test]
    fn operations_after_clear_data_report_instance_closed() {
        let (_dir, config) = temp_config(128);
        let mut mmkv = test_support::open(&config);
        mmkv.put("key1", Buffer::new("key1", 1)).unwrap();

        mmkv.clear_data().unwrap();
        assert!(!config.path.exists());

        assert_eq!(
            mmkv.put("key1", Buffer::new("key1", 2)),
            Err(InstanceClosed)
        );
        assert_eq!(mmkv.get::<i32>("key1"), Err(InstanceClosed));
        assert_eq!(mmkv.delete("key1"), Err(InstanceClosed));
        // Clearing an already-cleared instance is a no-op, not an error.
        assert_eq!(mmkv.clear_data(), Ok(()));
    }

    #[test]
    fn deleting_a_missing_key_is_ok_and_writes_no_tombstone() {
        let (_dir, config) = temp_config(256);
        let mut mmkv = test_support::open(&config);
        mmkv.put("key1", Buffer::new("key1", 1)).unwrap();
        let offset_before = write_offset_at(&config.path);

        assert_eq!(mmkv.delete("never_written"), Ok(()));

        assert_eq!(write_offset_at(&config.path), offset_before);
        assert_eq!(mmkv.get::<i32>("key1"), Ok(1));
        drop(mmkv);
        // And the reopened store agrees: no tombstone ever reached the file.
        assert_eq!(write_offset_at(&config.path), offset_before);
        assert_eq!(test_support::open(&config).get::<i32>("key1"), Ok(1));
    }
}
