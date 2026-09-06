//! Shared helpers for the unit tests that live inside `src/`.
//!
//! Test-only: the module is compiled under `#[cfg(test)]`. It exists so no unit test
//! hand-rolls a temp store, an encoder or a record length again. Record sizes always
//! come from the codec the current build flavour really uses (CRC-8 framing by default,
//! AES-EAX framing under `--features encryption`), so the same test is feature-neutral.

#![allow(dead_code)]

use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use tempfile::{TempDir, tempdir};

use crate::Result;
use crate::core::buffer::{Encoder, ProvideTypeToken, ToBytes};
use crate::core::config::Config;
#[cfg(not(feature = "encryption"))]
use crate::core::crc::CrcEncoder;
#[cfg(feature = "encryption")]
use crate::core::encrypt::Encryptor;
use crate::core::memory_map::MemoryMap;
use crate::core::mmkv_impl::MmkvImpl;

/// The 16-byte AES key (32 hex chars) every encrypted test store uses.
pub(crate) const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

/// Bytes reserved at the front of a store for the big-endian content length.
pub(crate) const HEADER_LEN: usize = size_of::<u64>();

/// The concrete encoder/decoder pair the library uses in this build flavour.
#[cfg(not(feature = "encryption"))]
pub(crate) type Codec = CrcEncoder;
#[cfg(feature = "encryption")]
pub(crate) type Codec = Encryptor;

/// A `Config` on a fresh `mmkv` file inside a temp dir. Keep the `TempDir` alive for the
/// duration of the test: dropping it removes the data file, the meta file and any trim
/// tmp files the writer created next to it.
pub(crate) fn temp_config(page_size: u64) -> (TempDir, Config) {
    let dir = tempdir().unwrap();
    let config = Config::new(&dir.path().join("mmkv"), page_size).unwrap();
    (dir, config)
}

/// (Re)open an `MmkvImpl` on `config.path` / `config.page_size`, passing [`TEST_KEY`]
/// under the encryption feature.
pub(crate) fn try_open(config: &Config) -> Result<MmkvImpl> {
    MmkvImpl::new(
        Config::new(&config.path, config.page_size)?,
        #[cfg(feature = "encryption")]
        TEST_KEY,
    )
}

/// [`try_open`], panicking on failure.
pub(crate) fn open(config: &Config) -> MmkvImpl {
    try_open(config).unwrap()
}

/// The codec the library would build for this path: `CrcEncoder`, or
/// `Encryptor::init(path, TEST_KEY)` (which creates/reads `path.meta`).
pub(crate) fn codec(_path: &Path) -> Codec {
    #[cfg(not(feature = "encryption"))]
    {
        CrcEncoder
    }
    #[cfg(feature = "encryption")]
    {
        Encryptor::init(_path, TEST_KEY).unwrap()
    }
}

/// [`codec`] boxed for call sites that only need to encode.
pub(crate) fn encoder(path: &Path) -> Box<dyn Encoder> {
    Box::new(codec(path))
}

/// Exact on-disk record length for `(key, value)` with the active feature.
/// Use it to size a page instead of hard-coding 17 (CRC) or 24 (AEAD) bytes.
pub(crate) fn record_len<T: ProvideTypeToken + ToBytes>(path: &Path, key: &str, value: T) -> usize {
    encoder(path)
        .encode_to_bytes(key, T::type_token().token, &value.to_bytes(), 0)
        .unwrap()
        .len()
}

/// The page size that holds exactly `records` records of `record_len` bytes.
pub(crate) fn page_for(records: usize, record_len: usize) -> u64 {
    (HEADER_LEN + records * record_len) as u64
}

/// The write offset (`HEADER_LEN` + stored content length) read straight from the file.
pub(crate) fn write_offset_at(path: &Path) -> usize {
    let file = open_rw(path);
    let len = file.metadata().unwrap().len();
    MemoryMap::new(&file, len).unwrap().write_offset().unwrap()
}

/// Overwrite the 8-byte big-endian content length in the file header.
pub(crate) fn set_content_len(path: &Path, content_len: u64) {
    let mut file = open_rw(path);
    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&content_len.to_be_bytes()).unwrap();
    file.sync_all().unwrap();
}

/// Append raw bytes at the current write offset and bump the header to cover them,
/// the way a corrupting writer would.
pub(crate) fn append_raw(path: &Path, bytes: &[u8]) {
    let offset = write_offset_at(path);
    let mut file = open_rw(path);
    file.seek(SeekFrom::Start(offset as u64)).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    set_content_len(path, (offset + bytes.len() - HEADER_LEN) as u64);
}

/// Pre-create `<name>.tmp.1 ..= <name>.tmp.count` next to `config.path` so that the next
/// `count` shadow-file trims fail at `create_new`. Call this only after the instance is
/// open: `Config::new` sweeps `<name>.tmp.*` siblings. Returns the paths so the test can
/// remove them again.
pub(crate) fn block_next_trims(config: &Config, count: usize) -> Vec<PathBuf> {
    let name = config
        .path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    (1..=count)
        .map(|seq| {
            let path = config.path.with_file_name(format!("{name}.tmp.{seq}"));
            std::fs::write(&path, b"").unwrap();
            path
        })
        .collect()
}

fn open_rw(path: &Path) -> std::fs::File {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap()
}
