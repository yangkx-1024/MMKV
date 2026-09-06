use crate::core::buffer::{Buffer, Encoder, SliceLoc};
use crate::core::config::Config;
#[cfg(feature = "encryption")]
use crate::core::encrypt::Encryptor;
use crate::core::io_looper::Executor;
use crate::core::memory_map::{MemoryMap, MmapHandle};
use crate::core::shared_state::SharedKvMap;
use crate::{Error, Result};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::sync::Arc;
use std::time::Instant;

const LOG_TAG: &str = "MMKV:IO";
const WRITE_OVERFLOW_ERR: &str = "write target overflowed";

pub struct IOWriter {
    config: Config,
    mm: MemoryMap,
    position: u32,
    need_trim: bool,
    trim_seq: u64,
    shared_kv: SharedKvMap,
    encoder: Box<dyn Encoder>,
    #[cfg(feature = "encryption")]
    encryptor: Encryptor,
}

impl Executor for IOWriter {}

impl IOWriter {
    pub fn new(
        config: Config,
        mm: MemoryMap,
        position: u32,
        shared_kv: SharedKvMap,
        encoder: Box<dyn Encoder>,
        #[cfg(feature = "encryption")] encryptor: Encryptor,
    ) -> Self {
        IOWriter {
            config,
            mm,
            position,
            need_trim: false,
            trim_seq: 0,
            shared_kv,
            encoder,
            #[cfg(feature = "encryption")]
            encryptor,
        }
    }

    // Flush the data to file, always running in one io thread, so don't need lock here
    pub fn write(&mut self, buffer: Buffer, duplicated: bool) -> Result<()> {
        let (key, type_token, value_owned) = match &buffer {
            Buffer::Owned { kv, .. } => (kv.key.clone(), kv.r#type, kv.value.clone()),
            Buffer::Slice(_) => {
                return Err(Error::IOError("write called with Slice buffer".to_string()));
            }
        };
        let data = self
            .encoder
            .encode_to_bytes(&key, type_token, &value_owned, self.position)?;
        let write_offset = self.mm.write_offset()?;
        let target_end = data
            .len()
            .checked_add(write_offset)
            .ok_or_else(|| Error::IOError(WRITE_OVERFLOW_ERR.to_string()))?;
        if duplicated {
            self.need_trim = true;
        }
        if target_end <= self.mm.len() {
            let record_start = write_offset;
            self.mm.append(&data)?;
            self.position += 1;
            if let Some(seq) = buffer.seq() {
                self.try_promote(&key, type_token, record_start, data.len(), seq);
            }
            return Ok(());
        }
        if self.need_trim {
            let time_start = Instant::now();
            info!(
                LOG_TAG,
                "start trim, current len {}",
                self.mm.write_offset()?
            );
            let snapshot = self.snapshot()?;
            info!(LOG_TAG, "snapshot finished in {:?}", time_start.elapsed());
            self.shadow_file_trim(&snapshot)?;
            self.need_trim = false;
            info!(
                LOG_TAG,
                "wrote {} items, new len {}, cost {:?}",
                self.position,
                self.mm.write_offset()?,
                time_start.elapsed()
            );
        } else {
            // expand and write
            self.ensure_capacity(data.len())?;
            let record_start = write_offset;
            self.mm.append(&data)?;
            self.position += 1;
            if let Some(seq) = buffer.seq() {
                self.try_promote(&key, type_token, record_start, data.len(), seq);
            }
        }
        Ok(())
    }

    /// After a successful append, promote the Owned entry to Slice if the seq still matches.
    fn try_promote(
        &self,
        key: &str,
        type_token: i32,
        record_start: usize,
        record_len: usize,
        seq: u64,
    ) {
        let Some(slice_loc) = SliceLoc::from_record(
            self.mm.base_ptr(),
            record_start,
            record_len,
            type_token,
            self.position - 1,
        ) else {
            return;
        };

        let mut kv_map = match self.shared_kv.kv_map.write() {
            Ok(g) => g,
            Err(_) => return,
        };
        match kv_map.get(key) {
            Some(Buffer::Owned {
                seq: current_seq, ..
            }) if *current_seq == seq => {
                kv_map.insert(key.to_string(), Buffer::Slice(slice_loc));
            }
            _ => {} // newer put arrived; leave as-is
        }
    }

    fn snapshot(&self) -> Result<HashMap<String, Buffer>> {
        let mmap_guard = self.shared_kv.mmap.load();
        let mmap: &MmapHandle = &mmap_guard;
        let kv_map = self
            .shared_kv
            .kv_map
            .read()
            .map_err(|e| Error::LockError(e.to_string()))?;
        let mut result = HashMap::with_capacity(kv_map.len());
        for (key, buf) in kv_map.iter() {
            let owned = match buf {
                Buffer::Owned { .. } => buf.clone(),
                Buffer::Slice(_) => match self.encoder.materialize_slice(mmap, buf) {
                    Some((type_token, value)) => Buffer::from_kv(key, type_token, value),
                    None => {
                        return Err(Error::IOError(format!(
                            "failed to materialize slice for key '{key}'"
                        )));
                    }
                },
            };
            result.insert(key.clone(), owned);
        }
        Ok(result)
    }

    /// Write `snapshot` to a fresh shadow file, atomically rename it over the live file,
    /// then swap `shared_kv.mmap` and `kv_map` under a single `kv_map.write()` lock so
    /// readers always see a consistent (Slice offsets, MmapHandle) pair.
    fn shadow_file_trim(&mut self, snapshot: &HashMap<String, Buffer>) -> Result<()> {
        // For encryption builds, generate a fresh nonce for the tmp file but do NOT
        // rotate the live stream yet — that happens only after the rename succeeds.
        // This ensures any mid-trim failure leaves self.encoder on the old nonce,
        // so subsequent appends to the still-live old file stay single-generation.
        #[cfg(feature = "encryption")]
        let pending = self.encryptor.prepare_new_nonce()?;

        // Build a unique tmp path alongside the live file.
        self.trim_seq += 1;
        let seq = self.trim_seq;
        let tmp_path = {
            let name = self
                .config
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            self.config.path.with_file_name(format!("{name}.tmp.{seq}"))
        };

        // Create and size the tmp file.
        let tmp_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .map_err(|e| Error::IOError(format!("failed to create tmp file: {e}")))?;
        let initial_size = self.mm.len() as u64;
        tmp_file
            .set_len(initial_size)
            .map_err(|e| Error::IOError(format!("failed to size tmp file: {e}")))?;
        let mut tmp_mm = MemoryMap::new(&tmp_file, initial_size)?;

        // Write snapshot entries and record new Slice locations.
        let mut new_slice_locs: HashMap<String, SliceLoc> = HashMap::with_capacity(snapshot.len());
        let mut new_position: u32 = 0;
        for (key, buffer) in snapshot {
            let (type_token, value_ref): (i32, &[u8]) = match buffer {
                Buffer::Owned { kv, .. } => (kv.r#type, kv.value.as_slice()),
                Buffer::Slice(_) => {
                    error!(
                        LOG_TAG,
                        "unexpected Slice in shadow_file_trim snapshot, skipping key {key}"
                    );
                    continue;
                }
            };
            #[cfg(feature = "encryption")]
            let bytes = self.encryptor.encode_with_pending(
                &pending,
                key,
                type_token,
                value_ref,
                new_position,
            )?;
            #[cfg(not(feature = "encryption"))]
            let bytes = self
                .encoder
                .encode_to_bytes(key, type_token, value_ref, new_position)?;

            // Expand the tmp file if necessary.
            let write_offset = tmp_mm.write_offset()?;
            let target_end = write_offset
                .checked_add(bytes.len())
                .ok_or_else(|| Error::IOError(WRITE_OVERFLOW_ERR.to_string()))?;
            if target_end > tmp_mm.len() {
                let new_size = target_end.next_multiple_of(self.config.page_size as usize) as u64;
                tmp_file
                    .set_len(new_size)
                    .map_err(|e| Error::IOError(format!("failed to expand tmp file: {e}")))?;
                tmp_mm = MemoryMap::new(&tmp_file, new_size)?;
            }

            let record_start = tmp_mm.write_offset()?;
            tmp_mm.append(&bytes)?;

            if let Some(loc) = SliceLoc::from_record(
                tmp_mm.base_ptr(),
                record_start,
                bytes.len(),
                type_token,
                new_position,
            ) {
                new_slice_locs.insert(key.clone(), loc);
            }
            new_position += 1;
        }

        // Flush and atomically replace the live file.
        tmp_mm.flush()?;
        tmp_file
            .sync_all()
            .map_err(|e| Error::IOError(format!("failed to sync tmp file: {e}")))?;
        // Commit point A: persist {current=new, previous=old} to the meta file.
        // The live stream is still on the old nonce; a failure here is safe because
        // both the data file and self.encoder remain on the same (old) generation.
        #[cfg(feature = "encryption")]
        self.encryptor.persist_pending_to_meta(&pending)?;
        // Commit point B: atomically replace the data file.
        std::fs::rename(&tmp_path, &self.config.path)
            .map_err(|e| Error::IOError(format!("failed to rename tmp file: {e}")))?;

        let new_handle = tmp_mm.to_handle();

        // Commit point C: activate the new nonce in-memory BEFORE the kv_map swap.
        // Readers hold kv_map.read() across decrypt; once they see new Slices (new-nonce
        // ciphertext), self.encryptor must already be on the new nonce so decrypt succeeds.
        // decrypt_with_fallback covers the brief window where old Slices are still in the
        // map (old ciphertext decrypts via previous_nonce = old).
        #[cfg(feature = "encryption")]
        self.encryptor.activate_pending(pending);

        // Atomically swap mmap and kv_map under a single write lock so that
        // readers (which take kv_map.read() before mmap.load()) always see a
        // consistent (Slice offsets, MmapHandle) pair.
        {
            let mut kv_map = self
                .shared_kv
                .kv_map
                .write()
                .map_err(|e| Error::LockError(e.to_string()))?;
            self.shared_kv.mmap.store(Arc::new(new_handle));
            let mut new_map = HashMap::with_capacity(kv_map.len());
            for (k, current_buf) in kv_map.iter() {
                match current_buf {
                    Buffer::Owned { .. } => {
                        // A put arrived after snapshot; keep it — IO thread will append it next.
                        new_map.insert(k.clone(), current_buf.clone());
                    }
                    Buffer::Slice(_) => {
                        // Replace stale Slice with one pointing at the new mmap.
                        if let Some(loc) = new_slice_locs.get(k) {
                            new_map.insert(k.clone(), Buffer::Slice(loc.clone()));
                        }
                        // Key absent from new_slice_locs means it was deleted between
                        // snapshot and now; omitting it from new_map is correct.
                    }
                }
            }
            *kv_map = new_map;
        }
        self.mm = tmp_mm;
        // Replace the file handle so ensure_capacity's set_len operates on the live file's fd.
        self.config.file = tmp_file;
        self.position = new_position;
        Ok(())
    }

    fn ensure_capacity(&mut self, incoming_len: usize) -> Result<()> {
        let write_offset = self.mm.write_offset()?;
        let required_len = write_offset
            .checked_add(incoming_len)
            .ok_or_else(|| Error::IOError(WRITE_OVERFLOW_ERR.to_string()))?;
        if required_len <= self.mm.len() {
            return Ok(());
        }
        let file_len = self.config.ensure_file_len(
            u64::try_from(required_len)
                .map_err(|_| Error::IOError(WRITE_OVERFLOW_ERR.to_string()))?,
        )?;
        self.mm = MemoryMap::new(&self.config.file, file_len)?;
        // Publish new mmap handle so readers see the expanded mapping.
        self.shared_kv.mmap.store(Arc::new(self.mm.to_handle()));
        Ok(())
    }

    pub fn remove_file(&mut self) -> Result<()> {
        self.config.remove_file()
    }
}

/// Implementation-level unit tests for the writer: append, expand, shadow-file trim and
/// the Owned -> Slice promotion of `shared_kv.kv_map` entries.
#[cfg(test)]
mod tests {
    use super::IOWriter;
    use crate::Error::KeyNotFound;
    use crate::core::buffer::{Buffer, ProvideTypeToken};
    use crate::core::config::Config;
    use crate::core::memory_map::MemoryMap;
    use crate::core::shared_state::{SharedKvMap, SharedState};
    use crate::core::test_support::{self, page_for, record_len, temp_config};
    use std::collections::HashMap;
    use std::path::Path;
    use tempfile::tempdir;

    fn make_writer(config: Config, mm: MemoryMap, shared_kv: SharedKvMap, path: &Path) -> IOWriter {
        let codec = test_support::codec(path);
        #[cfg(not(feature = "encryption"))]
        {
            IOWriter::new(config, mm, 0, shared_kv, Box::new(codec))
        }
        #[cfg(feature = "encryption")]
        {
            IOWriter::new(config, mm, 0, shared_kv, Box::new(codec.clone()), codec)
        }
    }

    fn new_shared_state(mm: &MemoryMap) -> SharedKvMap {
        SharedState::new(mm.to_handle(), HashMap::new())
    }

    fn insert(shared_kv: &SharedKvMap, buffer: Buffer) {
        shared_kv
            .kv_map
            .write()
            .unwrap()
            .insert(buffer.key().to_string(), buffer);
    }

    fn delete(shared_kv: &SharedKvMap, key: &str) {
        shared_kv.kv_map.write().unwrap().remove(key);
    }

    #[test]
    fn write_expands_until_large_record_fits() {
        let (_dir, config) = temp_config(64);
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap()).unwrap();
        let shared_kv = new_shared_state(&mm);
        let mut writer = make_writer(
            config.try_clone().unwrap(),
            mm,
            shared_kv.clone(),
            &config.path,
        );

        let large_value = vec![7u8; 256];
        let buffer = Buffer::new("large", large_value.as_slice());
        insert(&shared_kv, buffer.clone());
        writer.write(buffer, false).unwrap();

        let expected_len = writer.mm.write_offset().unwrap().next_multiple_of(64);
        assert_eq!(writer.mm.len(), expected_len);
        assert_eq!(config.file_size().unwrap() as usize, expected_len);
        assert_eq!(writer.position, 1);
        assert!(matches!(
            shared_kv.kv_map.read().unwrap().get("large").unwrap(),
            Buffer::Slice(_)
        ));

        let reopened = test_support::open(&config);
        assert_eq!(reopened.get::<Vec<u8>>("large").unwrap(), vec![7u8; 256]);
    }

    #[test]
    fn trim_uses_latest_len_after_expand() {
        let (_dir, config) = temp_config(96);
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap()).unwrap();
        let shared_kv = new_shared_state(&mm);
        let mut writer = make_writer(
            config.try_clone().unwrap(),
            mm,
            shared_kv.clone(),
            &config.path,
        );

        let value1 = vec![1u8; 40];
        let value2 = vec![2u8; 40];
        let buffer1 = Buffer::new("k1", value1.as_slice());
        let buffer2 = Buffer::new("k2", value2.as_slice());
        insert(&shared_kv, buffer1.clone());
        writer.write(buffer1, false).unwrap();
        insert(&shared_kv, buffer2.clone());
        writer.write(buffer2, false).unwrap();
        let initial_len = writer.mm.len();

        let updated = vec![3u8; 120];
        let buffer3 = Buffer::new("k1", updated.as_slice());
        insert(&shared_kv, buffer3.clone());
        writer.write(buffer3, true).unwrap();

        assert!(writer.mm.len() > initial_len);
        assert_eq!(writer.position, 2);
        // Verify values via reopen (kv_map entries may be Slice after shadow-file trim,
        // so we cannot use kv_value() which only works on Owned).
        let reopened = test_support::open(&config);
        assert_eq!(reopened.get::<Vec<u8>>("k1").unwrap(), vec![3u8; 120]);
        assert_eq!(reopened.get::<Vec<u8>>("k2").unwrap(), vec![2u8; 40]);
    }

    #[test]
    fn trim_rewrites_from_shared_snapshot_after_delete() {
        let (_dir, config) = temp_config(96);
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap()).unwrap();
        let shared_kv = new_shared_state(&mm);
        let mut writer = make_writer(
            config.try_clone().unwrap(),
            mm,
            shared_kv.clone(),
            &config.path,
        );

        let value1 = vec![1u8; 40];
        let value2 = vec![2u8; 40];
        let value3 = vec![3u8; 120];
        let buffer1 = Buffer::new("k1", value1.as_slice());
        let buffer2 = Buffer::new("k2", value2.as_slice());
        insert(&shared_kv, buffer1.clone());
        writer.write(buffer1, false).unwrap();
        insert(&shared_kv, buffer2.clone());
        writer.write(buffer2, false).unwrap();
        delete(&shared_kv, "k1");
        writer.write(Buffer::deleted_buffer("k1"), true).unwrap();
        let buffer3 = Buffer::new("k3", value3.as_slice());
        insert(&shared_kv, buffer3.clone());
        writer.write(buffer3, false).unwrap();

        assert_eq!(writer.position, 2);
        assert!(!shared_kv.kv_map.read().unwrap().contains_key("k1"));

        let reopened = test_support::open(&config);
        assert_eq!(reopened.get::<Vec<u8>>("k1"), Err(KeyNotFound));
        assert_eq!(reopened.get::<Vec<u8>>("k2").unwrap(), vec![2u8; 40]);
        assert_eq!(reopened.get::<Vec<u8>>("k3").unwrap(), vec![3u8; 120]);
    }

    #[test]
    #[cfg(feature = "encryption")]
    fn trim_rotates_nonce() {
        use std::fs;
        let (_dir, config) = temp_config(96);
        let meta_path = config.path.with_extension("meta");
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap()).unwrap();
        let shared_kv = new_shared_state(&mm);
        let mut writer = make_writer(
            config.try_clone().unwrap(),
            mm,
            shared_kv.clone(),
            &config.path,
        );

        let value1 = vec![1u8; 40];
        let value2 = vec![2u8; 40];
        let buffer1 = Buffer::new("k1", value1.as_slice());
        let buffer2 = Buffer::new("k2", value2.as_slice());
        insert(&shared_kv, buffer1.clone());
        writer.write(buffer1, false).unwrap();
        insert(&shared_kv, buffer2.clone());
        writer.write(buffer2, false).unwrap();

        let nonce_before = fs::read(&meta_path).unwrap();

        let updated = vec![3u8; 120];
        let buffer3 = Buffer::new("k1", updated.as_slice());
        insert(&shared_kv, buffer3.clone());
        writer.write(buffer3, true).unwrap();

        let nonce_after = fs::read(&meta_path).unwrap();
        assert_ne!(
            nonce_before, nonce_after,
            "nonce must rotate on rewrite_snapshot"
        );

        let reopened = test_support::open(&config);
        assert_eq!(reopened.get::<Vec<u8>>("k1").unwrap(), updated);
        assert_eq!(reopened.get::<Vec<u8>>("k2").unwrap(), value2);
    }

    /// Regression test for the mixed-generation corruption described in the nonce-rotation
    /// shadow-file race fix: if trim fails while creating the tmp file (which happens BEFORE
    /// the nonce commit in the fixed code, but AFTER the nonce rotation in the old code),
    /// subsequent appends to the old file must still use the old nonce so that the store
    /// reopens cleanly.
    ///
    /// Failure injection: pre-create "<file>.tmp.1" so that `create_new` in shadow_file_trim
    /// fails with EEXIST.  In the OLD code the nonce was rotated at the top of
    /// shadow_file_trim (before create_new), so the post-failure append used the NEW nonce
    /// on the OLD file, producing a mixed-generation store.  In the FIXED code prepare_new_nonce
    /// is pure in-memory and the meta is only persisted after sync_all, so the post-failure
    /// append still uses the OLD nonce — the store is single-generation and survives reopen.
    #[test]
    #[cfg(feature = "encryption")]
    fn trim_failure_does_not_produce_mixed_generation_store() {
        use std::fs;
        let (_dir, config) = temp_config(96);
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap()).unwrap();
        let shared_kv = new_shared_state(&mm);
        let mut writer = make_writer(
            config.try_clone().unwrap(),
            mm,
            shared_kv.clone(),
            &config.path,
        );

        // Write two large keys to fill the page so the next write triggers trim.
        let v1 = vec![1u8; 40];
        let v2 = vec![2u8; 40];
        insert(&shared_kv, Buffer::new("k1", v1.as_slice()));
        writer
            .write(Buffer::new("k1", v1.as_slice()), false)
            .unwrap();
        insert(&shared_kv, Buffer::new("k2", v2.as_slice()));
        writer
            .write(Buffer::new("k2", v2.as_slice()), false)
            .unwrap();

        // Block the trim by pre-creating the tmp file that shadow_file_trim will try to
        // create_new.  The failure happens at open() before any nonce state is committed.
        let blockers = test_support::block_next_trims(&config, 1);

        // A large write with duplicated=true triggers trim, which fails at create_new.
        let v3 = vec![3u8; 120];
        insert(&shared_kv, Buffer::new("k1", v3.as_slice()));
        let trim_result = writer.write(Buffer::new("k1", v3.as_slice()), true);
        assert!(
            trim_result.is_err(),
            "trim must fail when tmp file is pre-occupied"
        );

        // Remove the blocking file so the path is clean.
        for blocker in blockers {
            fs::remove_file(blocker).unwrap();
        }

        // A small append that fits in the current mmap.  Before the fix, the nonce was
        // already rotated in-memory (by before_rewrite at the top of shadow_file_trim),
        // so this append would encrypt with the NEW nonce onto the OLD file — producing a
        // mixed-generation store that drops this record on reopen.  After the fix, the
        // nonce is not activated until after the rename, so this append uses the OLD nonce.
        let v4 = vec![4u8; 10];
        insert(&shared_kv, Buffer::new("k2", v4.as_slice()));
        writer
            .write(Buffer::new("k2", v4.as_slice()), false)
            .unwrap();

        drop(writer);

        let reopened = test_support::open(&config);
        assert_eq!(
            reopened.get::<Vec<u8>>("k2").unwrap(),
            v4,
            "post-failure append must survive reopen without mixed-generation corruption"
        );
    }

    #[test]
    fn trim_reads_latest_shared_snapshot() {
        let (_dir, config) = temp_config(96);
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap()).unwrap();
        let shared_kv = new_shared_state(&mm);
        let mut writer = make_writer(
            config.try_clone().unwrap(),
            mm,
            shared_kv.clone(),
            &config.path,
        );

        let initial = vec![1u8; 40];
        let mid = vec![2u8; 120];
        let future = vec![3u8; 40];

        let buffer1 = Buffer::new("k1", initial.as_slice());
        insert(&shared_kv, buffer1.clone());
        writer.write(buffer1, false).unwrap();

        let mid_buffer = Buffer::new("k1", mid.as_slice());
        insert(&shared_kv, mid_buffer.clone());
        let future_buffer = Buffer::new("k1", future.as_slice());
        insert(&shared_kv, future_buffer.clone());

        writer.write(mid_buffer, true).unwrap();

        let reopened = test_support::open(&config);
        assert_eq!(reopened.get::<Vec<u8>>("k1").unwrap(), future);
    }

    /// Only `Owned` buffers carry the key and value the encoder needs; a `Slice` names
    /// bytes that are already in the file and must never be re-submitted for writing.
    #[test]
    fn writing_a_slice_buffer_is_rejected() {
        let (_dir, config) = temp_config(256);
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap()).unwrap();
        let shared_kv = new_shared_state(&mm);
        let mut writer = make_writer(
            config.try_clone().unwrap(),
            mm,
            shared_kv.clone(),
            &config.path,
        );

        let buffer = Buffer::new("k1", 1i32);
        insert(&shared_kv, buffer.clone());
        writer.write(buffer, false).unwrap();

        let promoted = shared_kv.kv_map.read().unwrap().get("k1").unwrap().clone();
        assert!(matches!(promoted, Buffer::Slice(_)));
        assert!(writer.write(promoted, false).is_err());
    }

    #[test]
    fn a_successful_append_promotes_the_map_entry_to_a_slice() {
        let (_dir, config) = temp_config(256);
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap()).unwrap();
        let shared_kv = new_shared_state(&mm);
        let mut writer = make_writer(
            config.try_clone().unwrap(),
            mm,
            shared_kv.clone(),
            &config.path,
        );

        let value = vec![5u8; 24];
        let buffer = Buffer::new("k1", value.as_slice()).with_seq(1);
        insert(&shared_kv, buffer.clone());
        assert!(matches!(
            shared_kv.kv_map.read().unwrap().get("k1").unwrap(),
            Buffer::Owned { .. }
        ));

        writer.write(buffer, false).unwrap();

        let promoted = shared_kv.kv_map.read().unwrap().get("k1").unwrap().clone();
        assert!(matches!(promoted, Buffer::Slice(_)));
        let mmap_guard = shared_kv.mmap.load();
        let (type_token, bytes) = writer
            .encoder
            .materialize_slice(&mmap_guard, &promoted)
            .expect("a promoted Slice must be readable from the mmap");
        assert_eq!(type_token, <&[u8] as ProvideTypeToken>::type_token().token);
        assert_eq!(bytes, value);
    }

    /// A put that lands while the previous one is still in the IO queue must win: the
    /// writer may only promote the entry it actually wrote.
    #[test]
    fn a_stale_promotion_never_overwrites_a_newer_put() {
        let (_dir, config) = temp_config(256);
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap()).unwrap();
        let shared_kv = new_shared_state(&mm);
        let mut writer = make_writer(
            config.try_clone().unwrap(),
            mm,
            shared_kv.clone(),
            &config.path,
        );

        let stale = Buffer::new("k1", 1i32).with_seq(1);
        let newest = Buffer::new("k1", 2i32).with_seq(2);
        insert(&shared_kv, newest);

        writer.write(stale, false).unwrap();

        let entry = shared_kv.kv_map.read().unwrap().get("k1").unwrap().clone();
        assert!(
            matches!(entry, Buffer::Owned { seq: 2, .. }),
            "expected the seq-2 Owned entry, got {entry:?}"
        );
    }

    #[test]
    fn expanding_publishes_a_bigger_mmap_and_keeps_old_slices_readable() {
        let (_dir, config) = temp_config(64);
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap()).unwrap();
        let shared_kv = new_shared_state(&mm);
        let mut writer = make_writer(
            config.try_clone().unwrap(),
            mm,
            shared_kv.clone(),
            &config.path,
        );

        let first = vec![1u8; 8];
        let buffer = Buffer::new("k1", first.as_slice());
        insert(&shared_kv, buffer.clone());
        writer.write(buffer, false).unwrap();
        let slice_before = shared_kv.kv_map.read().unwrap().get("k1").unwrap().clone();
        assert!(matches!(slice_before, Buffer::Slice(_)));
        let len_before = shared_kv.mmap.load().len();

        // A record that cannot fit forces an expand (nothing is pending a trim).
        let large = vec![2u8; 256];
        let buffer = Buffer::new("k2", large.as_slice());
        insert(&shared_kv, buffer.clone());
        writer.write(buffer, false).unwrap();

        let mmap_guard = shared_kv.mmap.load();
        assert!(
            mmap_guard.len() > len_before,
            "expand must publish a bigger handle: {} -> {}",
            len_before,
            mmap_guard.len()
        );
        // Slice offsets are relative to the mapping start, so the pre-expand entry still
        // points at the same record in the remapped file.
        let (_, bytes) = writer
            .encoder
            .materialize_slice(&mmap_guard, &slice_before)
            .expect("a Slice taken before the expand must still read");
        assert_eq!(bytes, first);
    }

    /// After deleting a key, the next trim rewrites only the live entries, so the write
    /// offset moves back below where it was before the trim.
    #[test]
    fn a_trim_after_a_delete_shrinks_the_write_offset() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mmkv");
        let value = vec![6u8; 16];
        let rec = record_len(&path, "k1", value.as_slice());
        let config = Config::new(&path, page_for(4, rec)).unwrap();
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap()).unwrap();
        let shared_kv = new_shared_state(&mm);
        let mut writer = make_writer(
            config.try_clone().unwrap(),
            mm,
            shared_kv.clone(),
            &config.path,
        );

        for key in ["k1", "k2", "k3", "k4"] {
            let buffer = Buffer::new(key, value.as_slice());
            insert(&shared_kv, buffer.clone());
            writer.write(buffer, false).unwrap();
        }
        let offset_before_trim = writer.mm.write_offset().unwrap();
        assert_eq!(offset_before_trim, page_for(4, rec) as usize);

        // The page is exactly full, so the tombstone cannot be appended and the writer
        // has to rewrite the live entries into a shadow file instead.
        delete(&shared_kv, "k1");
        writer.write(Buffer::deleted_buffer("k1"), true).unwrap();

        assert_eq!(writer.mm.write_offset().unwrap(), page_for(3, rec) as usize);
        assert!(writer.mm.write_offset().unwrap() < offset_before_trim);

        let reopened = test_support::open(&config);
        assert_eq!(reopened.get::<Vec<u8>>("k1"), Err(KeyNotFound));
        for key in ["k2", "k3", "k4"] {
            assert_eq!(reopened.get::<Vec<u8>>(key).unwrap(), value);
        }
    }
}
