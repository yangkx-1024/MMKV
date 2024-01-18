use crate::core::buffer::{Buffer, Decoder, Encoder};
use crate::core::config::Config;
#[cfg(feature = "encryption")]
use crate::core::encrypt::Encryptor;
use crate::core::io_looper::Callback;
use crate::core::memory_map::MemoryMap;
use crate::Error::EncodeFailed;
use std::collections::HashMap;
use std::time::Instant;

const LOG_TAG: &str = "MMKV:IO";

pub struct IOWriter {
    config: Config,
    mm: MemoryMap,
    need_trim: bool,
    encoder: Box<dyn Encoder>,
    decoder: Box<dyn Decoder>,
    #[cfg(feature = "encryption")]
    encryptor: Encryptor,
}

impl Callback for IOWriter {}

impl IOWriter {
    pub fn new(
        config: Config,
        mm: MemoryMap,
        encoder: Box<dyn Encoder>,
        decoder: Box<dyn Decoder>,
        #[cfg(feature = "encryption")] encryptor: Encryptor,
    ) -> Self {
        IOWriter {
            config,
            mm,
            need_trim: false,
            encoder,
            decoder,
            #[cfg(feature = "encryption")]
            encryptor,
        }
    }

    // Flash the data to file, always running in one io thread, so don't need lock here
    pub fn write(&mut self, buffer: Buffer, duplicated: bool) {
        let data = self.encoder.encode_to_bytes(&buffer).unwrap();
        let target_end = data.len() + self.mm.offset();
        let max_len = self.mm.len();
        if duplicated {
            self.need_trim = true;
        }
        if target_end <= max_len {
            self.mm.append(data).unwrap();
            return;
        }
        if self.need_trim {
            // rewrite the entire map
            #[cfg(feature = "encryption")]
            self.encryptor.reset();
            let time_start = Instant::now();
            info!(LOG_TAG, "start trim, current len {}", self.mm.offset());
            let mut snapshot: HashMap<String, Buffer> = HashMap::new();
            self.mm
                .iter(|bytes| self.decoder.decode_bytes(bytes))
                .for_each(|buffer| {
                    if let Some(data) = buffer {
                        snapshot.insert(data.key().to_string(), data);
                    }
                });
            snapshot.insert(buffer.key().to_string(), buffer);
            let mut count = 0;
            self.mm
                .reset()
                .map_err(|e| EncodeFailed(e.to_string()))
                .unwrap();
            #[cfg(feature = "encryption")]
            self.encryptor.reset();
            for buffer in snapshot.values() {
                let bytes = self.encoder.encode_to_bytes(buffer).unwrap();
                if self.mm.offset() + bytes.len() > max_len {
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
                self.mm.offset(),
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
        self.mm = MemoryMap::new(&self.config.file, self.config.file_size() as usize);
    }

    pub fn remove_file(&mut self) {
        self.config.remove_file();
        #[cfg(feature = "encryption")]
        self.encryptor.remove_file();
    }
}
