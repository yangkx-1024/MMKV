use crate::core::buffer::{Buffer, DecodeResult, SliceLoc};
use crate::core::memory_map::MemoryMap;
use std::collections::HashMap;

const LOG_TAG: &str = "MMKV:MemoryMap";

pub struct Iter<'a, F>
where
    F: Fn(&[u8], u32) -> crate::Result<DecodeResult>,
{
    mm: &'a MemoryMap,
    pub position: u32,
    start: usize,
    end: usize,
    decode: F,
}

impl MemoryMap {
    pub fn iter<F>(&self, decode: F) -> crate::Result<Iter<'_, F>>
    where
        F: Fn(&[u8], u32) -> crate::Result<DecodeResult>,
    {
        let start = self.content_start_offset();
        let end = self.write_offset()?;
        Ok(Iter {
            mm: self,
            position: 0,
            start,
            end,
            decode,
        })
    }
}

impl<F> Iter<'_, F>
where
    F: Fn(&[u8], u32) -> crate::Result<DecodeResult>,
{
    /// Decode every record between the content start and the write offset into a map
    /// of live entries. Returns `(map, record_count, end_offset)`, where `end_offset` is
    /// the byte offset right after the last record that could be framed. For an intact
    /// file it equals the write offset; anything after it is undecodable and the caller
    /// should discard it.
    pub fn into_map(mut self, mmap_base: usize) -> (HashMap<String, Buffer>, u32, usize) {
        let mut iter_count = 0u32;
        let mut map = HashMap::new();

        loop {
            let record_start = self.start;
            if self.start >= self.end {
                break;
            }
            let bytes = match self.mm.read(self.start..self.end) {
                Ok(b) => b,
                Err(e) => {
                    error!(LOG_TAG, "Failed to read memory map: {:?}", e);
                    break;
                }
            };
            let position = self.position;
            let decode_result = match (self.decode)(bytes, position) {
                Ok(r) => r,
                Err(e) => {
                    error!(LOG_TAG, "Failed to iter memory map: {:?}", e);
                    break;
                }
            };
            self.position += 1;
            iter_count += 1;
            let record_len = decode_result.len as usize;
            self.start += record_len;

            let buffer = match decode_result.buffer {
                Some(b) => b,
                None => continue,
            };

            if buffer.is_deleting() {
                // Tombstone: remove key. For Owned we have the key; Slice shouldn't appear here.
                if let Buffer::Owned { ref kv, .. } = buffer {
                    map.remove(kv.key.as_str());
                }
                continue;
            }

            // Build a Slice entry pointing into the mmap.
            let (key, type_token) = match &buffer {
                Buffer::Owned { kv, .. } => (kv.key.clone(), kv.r#type),
                Buffer::Slice(_) => continue, // shouldn't happen from decode
            };

            let slice_buf =
                SliceLoc::from_record(mmap_base, record_start, record_len, type_token, position)
                    .map(Buffer::Slice);
            map.insert(key, slice_buf.unwrap_or(buffer));
        }

        (map, iter_count, self.start.min(self.end))
    }
}

impl<F> Iterator for Iter<'_, F>
where
    F: Fn(&[u8], u32) -> crate::Result<DecodeResult>,
{
    type Item = Option<Buffer>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }
        let bytes = match self.mm.read(self.start..self.end) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!(LOG_TAG, "Failed to read memory map, reason: {:?}", e);
                return None;
            }
        };
        let decode_result = (self.decode)(bytes, self.position);
        self.position += 1;
        match decode_result {
            Ok(result) => {
                self.start += result.len as usize;
                Some(result.buffer)
            }
            Err(e) => {
                error!(LOG_TAG, "Failed to iter memory map, reason: {:?}", e);
                None
            }
        }
    }
}

/// Unit tests for record iteration and `into_map`, driven through the real codec of the
/// current build flavour so that `Buffer::Slice` construction is exercised too.
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs::{File, OpenOptions};
    use std::io::{Seek, SeekFrom, Write};

    use tempfile::{TempDir, tempdir};

    use crate::Result;
    use crate::core::buffer::{Buffer, Decoder, Encoder, FromBytes, ProvideTypeToken, ToBytes};
    use crate::core::memory_map::MemoryMap;
    use crate::core::test_support::{self, Codec, HEADER_LEN};

    /// A memory map on a real file plus the codec the library would use for it, so
    /// records are written exactly the way `IOWriter` writes them.
    struct Records {
        _dir: TempDir,
        file: File,
        mm: MemoryMap,
        codec: Codec,
        position: u32,
    }

    impl Records {
        fn new(len: u64) -> Self {
            let dir = tempdir().unwrap();
            let path = dir.path().join("mmkv");
            let codec = test_support::codec(&path);
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(&path)
                .unwrap();
            file.set_len(len).unwrap();
            let mm = MemoryMap::new(&file, len).unwrap();
            Records {
                _dir: dir,
                file,
                mm,
                codec,
                position: 0,
            }
        }

        /// Append one record, returning `(record_start, record_len)`.
        fn put<T: ProvideTypeToken + ToBytes>(&mut self, key: &str, value: T) -> (usize, usize) {
            self.append(key, T::type_token().token, &value.to_bytes())
        }

        /// Append a tombstone for `key`, returning `(record_start, record_len)`.
        fn delete(&mut self, key: &str) -> (usize, usize) {
            let tombstone = Buffer::deleted_buffer(key);
            let (type_token, value) = (tombstone.kv_type(), tombstone.kv_value().to_vec());
            self.append(key, type_token, &value)
        }

        fn append(&mut self, key: &str, type_token: i32, value: &[u8]) -> (usize, usize) {
            let bytes = self
                .codec
                .encode_to_bytes(key, type_token, value, self.position)
                .unwrap();
            let start = self.mm.write_offset().unwrap();
            self.mm.append(&bytes).unwrap();
            self.position += 1;
            (start, bytes.len())
        }

        /// Append bytes that are not a record, the way a truncated write would.
        fn append_raw(&mut self, bytes: &[u8]) -> usize {
            let start = self.mm.write_offset().unwrap();
            self.mm.append(bytes).unwrap();
            start
        }

        /// Corrupt one byte in place, leaving the surrounding framing intact.
        fn flip_byte(&mut self, offset: usize) {
            let original = self.mm.read(offset..offset + 1).unwrap()[0];
            let mut file = self.file.try_clone().unwrap();
            file.seek(SeekFrom::Start(offset as u64)).unwrap();
            file.write_all(&[original ^ 0xFF]).unwrap();
            file.sync_all().unwrap();
        }

        /// Run the real `Iter::into_map` over everything written so far.
        fn decode_all(&self) -> (HashMap<String, Buffer>, u32, usize) {
            let base = self.mm.base_ptr();
            self.mm
                .iter(|bytes, position| self.codec.decode_bytes(bytes, position))
                .unwrap()
                .into_map(base)
        }

        /// Read a value out of a decoded entry the way `MmkvImpl::get` would.
        fn value_of<T: ProvideTypeToken + FromBytes>(&self, buffer: &Buffer) -> Result<T> {
            let handle = self.mm.to_handle();
            #[cfg(not(feature = "encryption"))]
            {
                buffer.parse::<T>(&handle)
            }
            #[cfg(feature = "encryption")]
            {
                match buffer {
                    Buffer::Owned { .. } => buffer.parse::<T>(&handle),
                    Buffer::Slice(loc) => {
                        let plain = self
                            .codec
                            .decrypt_current(handle.read(loc.byte_range()), loc.position)?;
                        Buffer::from_encoded_bytes(&plain)?.parse::<T>(&handle)
                    }
                }
            }
        }

        fn write_offset(&self) -> usize {
            self.mm.write_offset().unwrap()
        }
    }

    #[test]
    fn distinct_records_all_load_with_their_values() {
        let mut records = Records::new(1024);
        for i in 0..10i32 {
            records.put(&format!("key{i}"), i);
        }

        let (map, count, end_offset) = records.decode_all();

        assert_eq!(count, 10);
        assert_eq!(map.len(), 10);
        assert_eq!(end_offset, records.write_offset());
        for i in 0..10i32 {
            let entry = map.get(&format!("key{i}")).unwrap();
            assert!(
                matches!(entry, Buffer::Slice(_)),
                "records read back from the mmap must be Slices"
            );
            assert_eq!(records.value_of::<i32>(entry), Ok(i));
        }
    }

    #[test]
    fn duplicate_keys_keep_the_last_value() {
        let mut records = Records::new(1024);
        records.put("key", 1i32);
        records.put("key", 2i32);
        records.put("key", 3i32);

        let (map, count, end_offset) = records.decode_all();

        assert_eq!(count, 3);
        assert_eq!(map.len(), 1);
        assert_eq!(end_offset, records.write_offset());
        assert_eq!(records.value_of::<i32>(map.get("key").unwrap()), Ok(3));
    }

    #[test]
    fn tombstones_remove_earlier_keys_and_still_count_as_records() {
        let mut records = Records::new(1024);
        records.put("kept", 1i32);
        records.put("dropped", 2i32);
        records.delete("dropped");
        records.delete("never_written");

        let (map, count, end_offset) = records.decode_all();

        assert_eq!(count, 4, "tombstones are records too");
        assert_eq!(map.len(), 1);
        assert!(!map.contains_key("dropped"));
        assert!(!map.contains_key("never_written"));
        assert_eq!(end_offset, records.write_offset());
        assert_eq!(records.value_of::<i32>(map.get("kept").unwrap()), Ok(1));
    }

    #[test]
    fn a_record_with_a_corrupted_body_is_skipped_and_later_records_still_load() {
        let mut records = Records::new(1024);
        records.put("first", 1i32);
        let (damaged_start, damaged_len) = records.put("second", 2i32);
        records.put("third", 3i32);

        // Flip a byte inside the frame, leaving the 4-byte length prefix intact so the
        // record still frames correctly and only its integrity check fails.
        records.flip_byte(damaged_start + damaged_len - 2);

        let (map, count, end_offset) = records.decode_all();

        assert_eq!(count, 3, "the damaged record is still framed and counted");
        assert!(!map.contains_key("second"));
        assert_eq!(end_offset, records.write_offset());
        assert_eq!(records.value_of::<i32>(map.get("first").unwrap()), Ok(1));
        assert_eq!(records.value_of::<i32>(map.get("third").unwrap()), Ok(3));
    }

    #[test]
    fn a_frame_running_past_the_content_stops_iteration() {
        let mut records = Records::new(1024);
        records.put("first", 1i32);
        records.put("second", 2i32);
        // A length prefix that claims far more bytes than the content holds.
        let garbage_start = records.append_raw(&[0xFF, 0xFF, 0xFF, 0xFF, 0xAA, 0xBB]);

        let (map, count, end_offset) = records.decode_all();

        assert_eq!(count, 2);
        assert_eq!(map.len(), 2);
        assert_eq!(
            end_offset, garbage_start,
            "iteration must stop at the start of the unframeable bytes"
        );
        assert_eq!(records.value_of::<i32>(map.get("first").unwrap()), Ok(1));
        assert_eq!(records.value_of::<i32>(map.get("second").unwrap()), Ok(2));
    }

    #[test]
    fn empty_content_yields_an_empty_map() {
        let records = Records::new(1024);

        let (map, count, end_offset) = records.decode_all();

        assert!(map.is_empty());
        assert_eq!(count, 0);
        assert_eq!(end_offset, HEADER_LEN);
        assert_eq!(end_offset, records.mm.content_start_offset());
    }
}
