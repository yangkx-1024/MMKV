use crate::core::buffer::{Buffer, Decoder, Encoder};
use crate::core::config::Config;
use crate::core::io_looper::Callback;
use crate::core::memory_map::MemoryMap;
use crate::Result;
use std::time::Instant;

const LOG_TAG: &str = "MMKV:IO";

pub struct IOWriter {
    config: Config,
    mm: MemoryMap,
    position: u32,
    need_trim: bool,
    encoder: Box<dyn Encoder>,
    decoder: Box<dyn Decoder>,
}

impl Callback for IOWriter {}

impl IOWriter {
    pub fn new(
        config: Config,
        mm: MemoryMap,
        position: u32,
        encoder: Box<dyn Encoder>,
        decoder: Box<dyn Decoder>,
    ) -> Self {
        IOWriter {
            config,
            mm,
            position,
            need_trim: false,
            encoder,
            decoder,
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
            // rewrite the entire map
            let time_start = Instant::now();
            info!(
                LOG_TAG,
                "start trim, current len {}",
                self.mm.write_offset()
            );
            info!(LOG_TAG, "start take snapshot");
            let (mut snapshot, _) = self
                .mm
                .iter(|bytes, position| self.decoder.decode_bytes(bytes, position))
                .into_map();
            if buffer.is_deleting() {
                snapshot.remove(buffer.key());
            } else {
                snapshot.insert(buffer.key().to_string(), buffer);
            }
            info!(LOG_TAG, "snapshot finished in {:?}", time_start.elapsed());
            self.mm.reset();
            self.position = 0;
            for buffer in snapshot.values() {
                let bytes = self.encoder.encode_to_bytes(buffer, self.position)?;
                self.ensure_capacity(bytes.len())?;
                self.mm.append(&bytes)?;
                self.position += 1;
            }
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
    use std::fs;
    use std::path::Path;

    #[cfg(feature = "encryption")]
    const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

    #[cfg(not(feature = "encryption"))]
    fn test_codec(
        _file_name: &str,
    ) -> (
        Box<dyn crate::core::buffer::Encoder>,
        Box<dyn crate::core::buffer::Decoder>,
    ) {
        (Box::new(CrcEncoderDecoder), Box::new(CrcEncoderDecoder))
    }

    #[cfg(feature = "encryption")]
    fn test_codec(
        file_name: &str,
    ) -> (
        Box<dyn crate::core::buffer::Encoder>,
        Box<dyn crate::core::buffer::Decoder>,
    ) {
        let encryptor = Encryptor::init(Path::new(file_name), TEST_KEY);
        (Box::new(encryptor.clone()), Box::new(encryptor))
    }

    #[test]
    fn write_expands_until_large_record_fits() {
        let file_name = "test_writer_large_record";
        let _ = fs::remove_file(file_name);
        let _ = fs::remove_file(format!("{file_name}.meta"));
        let config = Config::new(Path::new(file_name), 64).unwrap();
        let mm = MemoryMap::new(&config.file, config.file_size().unwrap() as usize).unwrap();
        let (encoder, decoder) = test_codec(file_name);
        let mut writer = IOWriter::new(config.try_clone().unwrap(), mm, 0, encoder, decoder);

        let large_value = vec![7u8; 256];
        writer
            .write(Buffer::new("large", large_value.as_slice()), false)
            .unwrap();

        assert!(writer.mm.len() >= writer.mm.write_offset());
        let (snapshot, count) = writer
            .mm
            .iter(|bytes, position| writer.decoder.decode_bytes(bytes, position))
            .into_map();
        assert_eq!(count, 1);
        assert_eq!(
            snapshot.get("large").unwrap().parse::<Vec<u8>>().unwrap(),
            large_value
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
        let (encoder, decoder) = test_codec(file_name);
        let mut writer = IOWriter::new(config, mm, 0, encoder, decoder);

        let value1 = vec![1u8; 40];
        let value2 = vec![2u8; 40];
        writer
            .write(Buffer::new("k1", value1.as_slice()), false)
            .unwrap();
        writer
            .write(Buffer::new("k2", value2.as_slice()), false)
            .unwrap();
        let initial_len = writer.mm.len();

        let updated = vec![3u8; 120];
        writer
            .write(Buffer::new("k1", updated.as_slice()), true)
            .unwrap();

        assert!(writer.mm.len() > initial_len);
        let (snapshot, count) = writer
            .mm
            .iter(|bytes, position| writer.decoder.decode_bytes(bytes, position))
            .into_map();
        assert_eq!(count, 2);
        assert_eq!(
            snapshot.get("k1").unwrap().parse::<Vec<u8>>().unwrap(),
            updated
        );
        assert_eq!(
            snapshot.get("k2").unwrap().parse::<Vec<u8>>().unwrap(),
            value2
        );

        writer.remove_file().unwrap();
        let _ = fs::remove_file(format!("{file_name}.meta"));
    }
}
