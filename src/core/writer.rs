use crate::core::buffer::{Buffer, Decoder, Encoder};
use crate::core::config::Config;
use crate::core::io_looper::Callback;
use crate::core::memory_map::MemoryMap;
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
    pub fn write(&mut self, buffer: Buffer, duplicated: bool) {
        let data = self
            .encoder
            .encode_to_bytes(&buffer, self.position)
            .unwrap();
        let target_end = data.len() + self.mm.write_offset();
        let max_len = self.mm.len();
        if duplicated {
            self.need_trim = true;
        }
        if target_end <= max_len {
            self.mm.append(data);
            self.position += 1;
            return;
        }
        if self.need_trim {
            // rewrite the entire map
            let time_start = Instant::now();
            info!(
                LOG_TAG,
                "start trim, current len {}",
                self.mm.write_offset()
            );
            info!(LOG_TAG,"start take snapshot");
            let (mut snapshot, _) = self
                .mm
                .iter(|bytes, position| self.decoder.decode_bytes(bytes, position))
                .into_map();
            if buffer.is_deleting() {
                snapshot.remove(buffer.key());
            } else {
                snapshot.insert(buffer.key().to_string(), buffer);
            }
            info!(LOG_TAG,"snapshot finished in {:?}", time_start.elapsed());
            self.mm.reset();
            self.position = 0;
            for buffer in snapshot.values() {
                let bytes = self.encoder.encode_to_bytes(buffer, self.position).unwrap();
                if self.mm.write_offset() + bytes.len() > max_len {
                    self.expand();
                }
                self.mm.append(bytes);
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
            self.expand();
            self.mm.append(data);
            self.position += 1;
        }
    }

    fn expand(&mut self) {
        self.config.expand();
        self.mm = MemoryMap::new(&self.config.file, self.config.file_size() as usize);
    }

    pub fn remove_file(&mut self) {
        self.config.remove_file();
    }
}
