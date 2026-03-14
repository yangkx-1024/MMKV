use crate::core::buffer::{Buffer, Encoder};
use crate::core::config::Config;
use crate::core::io_looper::Executor;
use crate::core::memory_map::MemoryMap;
use crate::core::shared_state::SharedKvMap;
use crate::{Error, Result};
use std::collections::HashMap;
use std::time::Instant;

const LOG_TAG: &str = "MMKV:IO";

pub struct IOWriter {
    config: Config,
    mm: MemoryMap,
    position: u32,
    need_trim: bool,
    shared_kv: SharedKvMap,
    encoder: Box<dyn Encoder>,
}

impl Executor for IOWriter {}

impl IOWriter {
    pub fn new(
        config: Config,
        mm: MemoryMap,
        position: u32,
        shared_kv: SharedKvMap,
        encoder: Box<dyn Encoder>,
    ) -> Self {
        IOWriter {
            config,
            mm,
            position,
            need_trim: false,
            shared_kv,
            encoder,
        }
    }

    // Flash the data to file, always running in one io thread, so don't need lock here
    pub fn write(&mut self, buffer: Buffer, duplicated: bool) -> Result<()> {
        let data = self.encoder.encode_to_bytes(&buffer, self.position)?;
        let target_end = data.len() + self.mm.write_offset();
        if duplicated {
            self.need_trim = true;
        }
        if target_end <= self.mm.len() {
            self.mm.append(&data)?;
            self.position += 1;
            return Ok(());
        }
        if self.need_trim {
            let time_start = Instant::now();
            info!(
                LOG_TAG,
                "start trim, current len {}",
                self.mm.write_offset()
            );
            let snapshot = self.snapshot()?;
            info!(LOG_TAG, "snapshot finished in {:?}", time_start.elapsed());
            self.rewrite_snapshot(&snapshot)?;
            self.need_trim = false;
            info!(
                LOG_TAG,
                "wrote {} items, new len {}, cost {:?}",
                self.position,
                self.mm.write_offset(),
                time_start.elapsed()
            );
        } else {
            // expand and write
            self.ensure_capacity(data.len())?;
            self.mm.append(&data)?;
            self.position += 1;
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<HashMap<String, Buffer>> {
        self.shared_kv
            .read()
            .map_err(|e| Error::LockError(e.to_string()))
            .map(|kv_map| kv_map.clone())
    }

    fn rewrite_snapshot(&mut self, snapshot: &HashMap<String, Buffer>) -> Result<()> {
        self.mm.reset();
        self.position = 0;
        for buffer in snapshot.values() {
            let bytes = self.encoder.encode_to_bytes(buffer, self.position)?;
            self.ensure_capacity(bytes.len())?;
            self.mm.append(&bytes)?;
            self.position += 1;
        }
        Ok(())
    }

    fn ensure_capacity(&mut self, incoming_len: usize) -> Result<()> {
        while self.mm.write_offset() + incoming_len > self.mm.len() {
            self.expand()?;
        }
        Ok(())
    }

    fn expand(&mut self) -> Result<()> {
        self.config.expand()?;
        self.mm = MemoryMap::new(&self.config.file, self.config.file_size()? as usize)?;
        Ok(())
    }

    pub fn remove_file(&mut self) -> Result<()> {
        self.config.remove_file()
    }
}

#[cfg(test)]
mod tests {
    use super::IOWriter;
    use crate::core::buffer::Buffer;
    use crate::core::config::Config;
    #[cfg(not(feature = "encryption"))]
    use crate::core::crc::CrcEncoderDecoder;
    #[cfg(feature = "encryption")]
    use crate::core::encrypt::Encryptor;
    use crate::core::memory_map::MemoryMap;
    use crate::core::mmkv_impl::MmkvImpl;
    use crate::core::shared_state::{new_shared_kv_map, SharedKvMap};
    use crate::Error::KeyNotFound;
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    #[cfg(feature = "encryption")]
    const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

    #[cfg(not(feature = "encryption"))]
    fn test_encoder(_file_name: &str) -> Box<dyn crate::core::buffer::Encoder> {
        Box::new(CrcEncoderDecoder)
    }

    #[cfg(feature = "encryption")]
    fn test_encoder(file_name: &str) -> Box<dyn crate::core::buffer::Encoder> {
        let encryptor = Encryptor::init(Path::new(file_name), TEST_KEY);
        Box::new(encryptor)
    }

    fn reopen_mmkv(config: &Config) -> MmkvImpl {
        MmkvImpl::new(
            config.try_clone().unwrap(),
            #[cfg(feature = "encryption")]
            TEST_KEY,
        )
        .unwrap()
    }

    fn new_shared_state() -> SharedKvMap {
        new_shared_kv_map(HashMap::new())
    }

    fn insert(shared_kv: &SharedKvMap, buffer: Buffer) {
        shared_kv
            .write()
            .unwrap()
            .insert(buffer.key().to_string(), buffer);
    }

    fn delete(shared_kv: &SharedKvMap, key: &str) {
        shared_kv.write().unwrap().remove(key);
    }

    #[test]
    fn write_expands_until_large_record_fits() {
        let file_name = "test_writer_large_record";
        let _ = fs::remove_file(file_name);
        let _ = fs::remove_file(format!("{file_name}.meta"));
        let config = Config::new(Path::new(file_name), 64).unwrap();
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap() as usize).unwrap();
        let encoder = test_encoder(file_name);
        let shared_kv = new_shared_state();
        let mut writer = IOWriter::new(
            config.try_clone().unwrap(),
            mm,
            0,
            shared_kv.clone(),
            encoder,
        );

        let large_value = vec![7u8; 256];
        let buffer = Buffer::new("large", large_value.as_slice());
        insert(&shared_kv, buffer.clone());
        writer.write(buffer, false).unwrap();

        assert!(writer.mm.len() >= writer.mm.write_offset());
        assert_eq!(writer.position, 1);
        assert_eq!(
            shared_kv
                .read()
                .unwrap()
                .get("large")
                .unwrap()
                .parse::<Vec<u8>>()
                .unwrap(),
            large_value
        );

        let reopened = reopen_mmkv(&config);
        assert_eq!(
            reopened.get("large").unwrap().parse::<Vec<u8>>().unwrap(),
            vec![7u8; 256]
        );

        writer.remove_file().unwrap();
        let _ = fs::remove_file(format!("{file_name}.meta"));
    }

    #[test]
    fn trim_uses_latest_len_after_expand() {
        let file_name = "test_writer_trim_expand";
        let _ = fs::remove_file(file_name);
        let _ = fs::remove_file(format!("{file_name}.meta"));
        let config = Config::new(Path::new(file_name), 96).unwrap();
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap() as usize).unwrap();
        let encoder = test_encoder(file_name);
        let shared_kv = new_shared_state();
        let mut writer = IOWriter::new(
            config.try_clone().unwrap(),
            mm,
            0,
            shared_kv.clone(),
            encoder,
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
        assert_eq!(
            shared_kv
                .read()
                .unwrap()
                .get("k1")
                .unwrap()
                .parse::<Vec<u8>>()
                .unwrap(),
            updated
        );
        assert_eq!(
            shared_kv
                .read()
                .unwrap()
                .get("k2")
                .unwrap()
                .parse::<Vec<u8>>()
                .unwrap(),
            value2
        );

        let reopened = reopen_mmkv(&config);
        assert_eq!(
            reopened.get("k1").unwrap().parse::<Vec<u8>>().unwrap(),
            vec![3u8; 120]
        );
        assert_eq!(
            reopened.get("k2").unwrap().parse::<Vec<u8>>().unwrap(),
            vec![2u8; 40]
        );

        writer.remove_file().unwrap();
        let _ = fs::remove_file(format!("{file_name}.meta"));
    }

    #[test]
    fn trim_rewrites_from_shared_snapshot_after_delete() {
        let file_name = "test_writer_delete_trim";
        let _ = fs::remove_file(file_name);
        let _ = fs::remove_file(format!("{file_name}.meta"));
        let config = Config::new(Path::new(file_name), 96).unwrap();
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap() as usize).unwrap();
        let encoder = test_encoder(file_name);
        let shared_kv = new_shared_state();
        let mut writer = IOWriter::new(
            config.try_clone().unwrap(),
            mm,
            0,
            shared_kv.clone(),
            encoder,
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
        assert!(!shared_kv.read().unwrap().contains_key("k1"));

        let reopened = reopen_mmkv(&config);
        assert_eq!(reopened.get("k1"), Err(KeyNotFound));
        assert_eq!(
            reopened.get("k2").unwrap().parse::<Vec<u8>>().unwrap(),
            vec![2u8; 40]
        );
        assert_eq!(
            reopened.get("k3").unwrap().parse::<Vec<u8>>().unwrap(),
            vec![3u8; 120]
        );

        writer.remove_file().unwrap();
        let _ = fs::remove_file(format!("{file_name}.meta"));
    }

    #[test]
    fn trim_reads_latest_shared_snapshot() {
        let file_name = "test_writer_trim_latest_shared_snapshot";
        let _ = fs::remove_file(file_name);
        let _ = fs::remove_file(format!("{file_name}.meta"));
        let config = Config::new(Path::new(file_name), 96).unwrap();
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap() as usize).unwrap();
        let encoder = test_encoder(file_name);
        let shared_kv = new_shared_state();
        let mut writer = IOWriter::new(
            config.try_clone().unwrap(),
            mm,
            0,
            shared_kv.clone(),
            encoder,
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

        let reopened = reopen_mmkv(&config);
        assert_eq!(
            reopened.get("k1").unwrap().parse::<Vec<u8>>().unwrap(),
            future
        );

        writer.remove_file().unwrap();
        let _ = fs::remove_file(format!("{file_name}.meta"));
    }
}
