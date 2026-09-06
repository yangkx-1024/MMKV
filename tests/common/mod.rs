//! Helpers shared by the integration tests.
//!
//! Everything here goes through the public API (`mmkv::MMKV`, `mmkv::Error`,
//! `mmkv::LogLevel`) plus `tempfile` and std: `src/core/test_support.rs` is
//! crate-private and cannot be reached from `tests/`.
//!
//! The two build flavours differ only in the record framing (CRC-8 by default, AES-EAX
//! under `--features encryption`), so the encryption key is hidden inside [`Store::open`]
//! and record sizes are always *measured* ([`record_len`]) instead of hard-coded.
//!
//! Every store lives in its own [`tempfile::TempDir`]; nothing is ever written into the
//! repository. Tests inside one integration binary run in parallel threads, so each test
//! must build its own [`Store`].

// Each integration binary includes this module and uses a different part of it.
#![allow(dead_code)]

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Once;

use mmkv::{LogLevel, MMKV};
use tempfile::TempDir;

/// The 16-byte AES key (32 hex chars) every encrypted test store uses.
pub const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

/// Bytes reserved at the front of a store for the big-endian content length.
pub const HEADER_LEN: usize = size_of::<u64>();

/// The file `MMKV` creates inside the directory it is given.
pub const DATA_FILE_NAME: &str = "mini_mmkv";

/// A temp directory holding exactly one store. Dropping it removes the data file, the
/// meta file and any trim tmp file left next to them.
pub struct Store {
    dir: TempDir,
}

impl Default for Store {
    fn default() -> Self {
        Store::new()
    }
}

impl Store {
    pub fn new() -> Self {
        // `set_log_level` is process-wide; every binary quiets MMKV exactly once.
        static LOG_LEVEL: Once = Once::new();
        LOG_LEVEL.call_once(|| MMKV::set_log_level(LogLevel::Warn));
        Store {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    pub fn dir(&self) -> &Path {
        self.dir.path()
    }

    pub fn dir_str(&self) -> &str {
        self.dir.path().to_str().unwrap()
    }

    /// A handle on this store. Handles for the same dir share one cached instance while
    /// any of them is alive, so drop every handle to force a reopen from disk.
    pub fn open(&self) -> MMKV {
        self.try_open().unwrap()
    }

    pub fn try_open(&self) -> mmkv::Result<MMKV> {
        open_dir(self.dir_str())
    }

    /// Open with a key of the test's choosing (encryption only).
    #[cfg(feature = "encryption")]
    pub fn try_open_with_key(&self, key: &str) -> mmkv::Result<MMKV> {
        MMKV::new(self.dir_str(), key)
    }

    pub fn data_file(&self) -> PathBuf {
        self.dir().join(DATA_FILE_NAME)
    }

    pub fn meta_file(&self) -> PathBuf {
        self.dir().join(format!("{DATA_FILE_NAME}.meta"))
    }

    /// Length of the data file. Right after the first open this is the page size.
    pub fn file_len(&self) -> u64 {
        fs::metadata(self.data_file()).unwrap().len()
    }

    /// The big-endian content length stored in the first 8 bytes of the data file.
    pub fn content_len(&self) -> u64 {
        let mut header = [0u8; HEADER_LEN];
        File::open(self.data_file())
            .unwrap()
            .read_exact(&mut header)
            .unwrap();
        u64::from_be_bytes(header)
    }

    /// Overwrite the content length in the header, the way a torn write would.
    pub fn set_content_len(&self, len: u64) {
        let mut file = self.open_rw();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&len.to_be_bytes()).unwrap();
        file.sync_all().unwrap();
    }

    /// Append raw bytes at the current write offset and bump the header to cover them,
    /// so the store believes those bytes are live records.
    pub fn append_raw(&self, bytes: &[u8]) {
        let content_len = self.content_len();
        let offset = HEADER_LEN as u64 + content_len;
        let mut file = self.open_rw();
        file.seek(SeekFrom::Start(offset)).unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
        self.set_content_len(content_len + bytes.len() as u64);
    }

    /// Craft a data file from scratch: `content_len` in the header, `body` right after
    /// it, zero-padded to `file_len` bytes.
    pub fn write_raw_store(&self, content_len: u64, body: &[u8], file_len: usize) {
        let mut bytes = Vec::with_capacity(file_len.max(HEADER_LEN + body.len()));
        bytes.extend_from_slice(&content_len.to_be_bytes());
        bytes.extend_from_slice(body);
        bytes.resize(file_len.max(bytes.len()), 0);
        fs::write(self.data_file(), &bytes).unwrap();
    }

    /// Occupy `mini_mmkv.tmp.1 ..= mini_mmkv.tmp.count` so the next `count` shadow-file
    /// trims fail at `create_new`. Call this only after the instance is open: opening a
    /// store sweeps its `tmp.*` siblings. Returns the paths so the test can free them.
    pub fn block_next_trims(&self, count: usize) -> Vec<PathBuf> {
        (1..=count)
            .map(|seq| {
                let path = self.dir().join(format!("{DATA_FILE_NAME}.tmp.{seq}"));
                fs::write(&path, b"").unwrap();
                path
            })
            .collect()
    }

    /// Byte-for-byte copy of everything that makes up the store on disk.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            data: fs::read(self.data_file()).unwrap(),
            meta: fs::read(self.meta_file()).ok(),
        }
    }

    /// Put the store back to the state [`Store::snapshot`] captured.
    pub fn restore(&self, snapshot: &Snapshot) {
        fs::write(self.data_file(), &snapshot.data).unwrap();
        match &snapshot.meta {
            Some(meta) => fs::write(self.meta_file(), meta).unwrap(),
            None => {
                let _ = fs::remove_file(self.meta_file());
            }
        }
    }

    fn open_rw(&self) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.data_file())
            .unwrap()
    }
}

/// Open a store at an arbitrary directory, applying [`TEST_KEY`] under the encryption
/// feature. [`Store`] owns its directory and deletes it on drop, so the crash tests —
/// whose child process is handed a path by its parent — go through this instead.
pub fn open_dir(dir: &str) -> mmkv::Result<MMKV> {
    MMKV::new(
        dir,
        #[cfg(feature = "encryption")]
        TEST_KEY,
    )
}

/// See [`Store::snapshot`].
pub struct Snapshot {
    pub data: Vec<u8>,
    pub meta: Option<Vec<u8>>,
}

/// `len` bytes of `fill`.
pub fn bytes(len: usize, fill: u8) -> Vec<u8> {
    vec![fill; len]
}

/// The exact on-disk size of one `(key, value)` record in the current build flavour,
/// measured by writing it into a throwaway store. Use it to size a page instead of
/// hard-coding 17 (CRC framing) or 24 (AEAD framing) bytes.
pub fn record_len(key: &str, value: &[u8]) -> u64 {
    let store = Store::new();
    let mmkv = store.open();
    mmkv.put(key, value).unwrap();
    let len = store.content_len();
    drop(mmkv);
    len
}

/// Small deterministic RNG (xorshift64*) so the model and byte-flip tests are
/// reproducible without pulling in a dependency.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // xorshift64* is undefined for a zero state.
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A value in `0..n`. `n` must be non-zero.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}
