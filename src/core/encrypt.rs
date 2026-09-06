use aead_stream::{NewStream, StreamBE32, StreamPrimitive};
use aes::Aes128;
use eax::Eax;
use eax::aead::consts::U8;
use eax::aead::{KeyInit, Payload};
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::Error::{DataInvalid, DecryptFailed, EncryptFailed};
use crate::Result;
use crate::core::buffer::{
    Buffer, DecodeResult, Decoder, Encoder, decode_kv_type_value, encode_kv_bytes, split_frame,
};
use crate::core::config::{parent_dir, sync_parent_dir};

const LOG_TAG: &str = "MMKV:Encrypt";
const NONCE_LEN: usize = 11;
const META_FILE_LEN_WITH_PREVIOUS: usize = NONCE_LEN * 2;
/// Authentication tag appended to every ciphertext (`U8` below).
const TAG_LEN: usize = 8;

type Aes128Eax = Eax<Aes128, U8>;
type Stream = StreamBE32<Aes128Eax>;

#[derive(Clone)]
pub struct Encryptor {
    pub meta_file_path: PathBuf,
    encryptor: Arc<RwLock<StreamWrapper>>,
}

struct StreamWrapper {
    stream: Stream,
    key: [u8; 16],
    current_nonce: [u8; NONCE_LEN],
    previous_nonce: Option<[u8; NONCE_LEN]>,
}

/// Ephemeral nonce + pre-built stream for shadow-file trim encoding.
/// Created before writing the tmp file; persisted to the meta file and activated
/// in-memory only after the tmp file is durably renamed over the live file,
/// so any mid-trim failure leaves both the data file and `self.encoder`'s stream
/// on the same (old) generation.
pub(crate) struct PendingNonce {
    nonce: [u8; NONCE_LEN],
    stream: Stream,
}

impl Encryptor {
    /// `key` must be 16 bytes as a 32-character hex string. Fails instead of panicking on
    /// a malformed key or when the meta file cannot be created, since this runs on the
    /// host's startup path.
    pub fn init(file_path: &Path, key: &str) -> Result<Self> {
        let decoded_key = Encryptor::decode_key(key)?;
        let meta_file_path = Encryptor::resolve_meta_file_path(file_path);
        let encryptor = StreamWrapper::init(decoded_key, &meta_file_path)?;
        Ok(Encryptor {
            meta_file_path,
            encryptor: Arc::new(RwLock::new(encryptor)),
        })
    }

    /// Whether `key` is the key this encryptor was initialised with. Compares the decoded
    /// bytes, so hex case does not matter. A malformed `key` is an error, not `false`.
    pub fn key_matches(&self, key: &str) -> Result<bool> {
        let decoded_key = Encryptor::decode_key(key)?;
        let inner = self
            .encryptor
            .read()
            .map_err(|e| EncryptFailed(e.to_string()))?;
        Ok(inner.key == decoded_key)
    }

    fn decode_key(key: &str) -> Result<[u8; 16]> {
        hex::decode(key)
            .ok()
            .and_then(|bytes| bytes.as_slice().try_into().ok())
            .ok_or_else(|| {
                EncryptFailed("key must be a 32-character hex string (16 bytes)".to_string())
            })
    }

    fn resolve_meta_file_path(path: &Path) -> PathBuf {
        let meta_ext = match path.extension() {
            Some(ext) => format!("{}.meta", ext.to_string_lossy()),
            None => "meta".to_string(),
        };
        path.with_extension(meta_ext)
    }

    #[cfg(test)]
    pub fn rotate_nonce(&self) -> Result<()> {
        self.encryptor
            .write()
            .map_err(|e| EncryptFailed(e.to_string()))?
            .rotate(&self.meta_file_path)
    }

    pub fn recover_current_nonce(&self, data: &[u8]) -> Result<()> {
        self.encryptor
            .write()
            .map_err(|e| EncryptFailed(e.to_string()))?
            .recover_current_nonce(data, &self.meta_file_path)
    }

    /// Decrypt bytes using the current nonce, falling back to the previous nonce on failure.
    /// The fallback handles the race window during shadow-file trim where the nonce is
    /// rotated before the kv_map atomic swap replaces all Slices with new-nonce offsets.
    pub fn decrypt_current(&self, ciphertext: &[u8], position: u32) -> Result<Vec<u8>> {
        self.encryptor
            .read()
            .map_err(|e| DecryptFailed(e.to_string()))?
            .decrypt_with_fallback(ciphertext.to_vec(), position)
    }

    /// Generate a fresh random nonce and a pre-built ephemeral stream bound to it.
    /// Pure in-memory — touches neither the meta file nor the live stream in `self.encryptor`.
    pub(crate) fn prepare_new_nonce(&self) -> Result<PendingNonce> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).expect("getrandom failed");
        let inner = self
            .encryptor
            .read()
            .map_err(|e| EncryptFailed(e.to_string()))?;
        let stream = StreamWrapper::build_stream(&inner.key, &nonce);
        Ok(PendingNonce { nonce, stream })
    }

    /// Encode a single KV record using the ephemeral stream in `pending` instead of the
    /// live stream. Used to fill the shadow tmp file before commit.
    pub(crate) fn encode_with_pending(
        &self,
        pending: &PendingNonce,
        key: &str,
        type_token: i32,
        value: &[u8],
        position: u32,
    ) -> Result<Vec<u8>> {
        if position == Stream::COUNTER_MAX {
            return Err(EncryptFailed(String::from("counter overflow")));
        }
        let kv_bytes = encode_kv_bytes(key, type_token, value);
        let crypt_bytes = pending
            .stream
            .encrypt(position, false, Payload::from(kv_bytes.as_slice()))
            .map_err(|e| EncryptFailed(e.to_string()))?;
        let len = crypt_bytes.len() as u32;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(&crypt_bytes);
        Ok(data)
    }

    /// Atomically persist `{current = pending.nonce, previous = old_nonce}` to the meta file.
    /// Does NOT switch the in-memory stream; that is deferred to `activate_pending`.
    pub(crate) fn persist_pending_to_meta(&self, pending: &PendingNonce) -> Result<()> {
        let inner = self
            .encryptor
            .read()
            .map_err(|e| EncryptFailed(e.to_string()))?;
        StreamWrapper::write_meta_file(
            &self.meta_file_path,
            &pending.nonce,
            Some(&inner.current_nonce),
        )
    }

    /// Install `pending` as the live in-memory stream. Call only after the renamed data file
    /// is visible at the live path so that any subsequent encode uses the new nonce.
    pub(crate) fn activate_pending(&self, pending: PendingNonce) {
        if let Ok(mut inner) = self.encryptor.write() {
            let old_nonce = inner.current_nonce;
            inner.activate_nonce(pending.nonce, old_nonce);
        }
    }
}

impl StreamWrapper {
    fn init(key: [u8; 16], meta_file_path: &PathBuf) -> Result<Self> {
        if meta_file_path.exists() {
            StreamWrapper::new_with_nonce(key, meta_file_path)
        } else {
            StreamWrapper::new(key, meta_file_path)
        }
    }

    fn new(key: [u8; 16], meta_file_path: &Path) -> Result<Self> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).expect("getrandom failed");
        Self::write_meta_file(meta_file_path, &nonce, None)?;
        Ok(StreamWrapper {
            stream: Self::build_stream(&key, &nonce),
            key,
            current_nonce: nonce,
            previous_nonce: None,
        })
    }

    fn new_with_nonce(key: [u8; 16], meta_file_path: &PathBuf) -> Result<Self> {
        let error_handle = |reason: String| -> Result<Self> {
            error!(LOG_TAG, "filed to read nonce, reason: {:?}", reason);
            warn!(
                LOG_TAG,
                "delete meta file due to previous reason, which may cause mmkv drop all encrypted data"
            );
            let _ = fs::remove_file(meta_file_path);
            StreamWrapper::new(key, meta_file_path)
        };
        let mut nonce_file = match OpenOptions::new().read(true).open(meta_file_path) {
            Ok(file) => file,
            Err(e) => return error_handle(format!("{:?}", e)),
        };
        let mut nonce_bytes = Vec::<u8>::new();
        match nonce_file.read_to_end(&mut nonce_bytes) {
            Ok(len) if len != NONCE_LEN && len != META_FILE_LEN_WITH_PREVIOUS => {
                return error_handle("meta file corruption".to_string());
            }
            Err(e) => return error_handle(format!("{:?}", e)),
            _ => {}
        }
        let current_nonce: [u8; NONCE_LEN] = match nonce_bytes
            .get(..NONCE_LEN)
            .and_then(|bytes| bytes.try_into().ok())
        {
            Some(nonce) => nonce,
            None => return error_handle("meta file corruption".to_string()),
        };
        let previous_nonce = nonce_bytes
            .get(NONCE_LEN..META_FILE_LEN_WITH_PREVIOUS)
            .and_then(|bytes| bytes.try_into().ok());
        Ok(StreamWrapper {
            stream: Self::build_stream(&key, &current_nonce),
            key,
            current_nonce,
            previous_nonce,
        })
    }

    #[cfg(test)]
    fn rotate(&mut self, meta_file_path: &Path) -> Result<()> {
        let previous_nonce = self.current_nonce;
        let mut current_nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut current_nonce).expect("getrandom failed");
        Self::write_meta_file(meta_file_path, &current_nonce, Some(&previous_nonce))?;
        // Replace in-memory stream only after the new nonce pair is safely on disk.
        self.activate_nonce(current_nonce, previous_nonce);
        Ok(())
    }

    fn recover_current_nonce(&mut self, data: &[u8], meta_file_path: &Path) -> Result<()> {
        if data.len() < size_of::<u32>() || self.can_decode_first_record(data, &self.current_nonce)
        {
            return Ok(());
        }
        let Some(previous_nonce) = self.previous_nonce else {
            return Ok(());
        };
        if !self.can_decode_first_record(data, &previous_nonce) {
            return Ok(());
        }
        let current_nonce = self.current_nonce;
        Self::write_meta_file(meta_file_path, &previous_nonce, Some(&current_nonce))?;
        self.activate_nonce(previous_nonce, current_nonce);
        Ok(())
    }

    fn activate_nonce(&mut self, new_nonce: [u8; NONCE_LEN], old_nonce: [u8; NONCE_LEN]) {
        self.stream = Self::build_stream(&self.key, &new_nonce);
        self.current_nonce = new_nonce;
        self.previous_nonce = Some(old_nonce);
    }

    fn build_stream(key: &[u8; 16], nonce: &[u8; NONCE_LEN]) -> Stream {
        let cipher = Aes128Eax::new(key.into());
        StreamBE32::from_aead(cipher, nonce.into())
    }

    fn can_decode_first_record(&self, data: &[u8], nonce: &[u8; NONCE_LEN]) -> bool {
        let Ok((bytes_to_decode, _)) = split_frame(data) else {
            return false;
        };
        let decrypted = match Self::build_stream(&self.key, nonce).decrypt(
            0,
            false,
            Payload::from(bytes_to_decode),
        ) {
            Ok(decrypted) => decrypted,
            Err(_) => return false,
        };
        Buffer::from_encoded_bytes(decrypted.as_slice()).is_ok()
    }

    fn write_meta_file(
        meta_file_path: &Path,
        current_nonce: &[u8; NONCE_LEN],
        previous_nonce: Option<&[u8; NONCE_LEN]>,
    ) -> Result<()> {
        let mut meta_bytes = Vec::with_capacity(match previous_nonce {
            Some(_) => META_FILE_LEN_WITH_PREVIOUS,
            None => NONCE_LEN,
        });
        meta_bytes.extend_from_slice(current_nonce);
        if let Some(previous_nonce) = previous_nonce {
            meta_bytes.extend_from_slice(previous_nonce);
        }

        let (tmp_path, mut nonce_file) = Self::open_temp_meta_file(meta_file_path)?;
        let write_result = (|| -> Result<()> {
            nonce_file
                .write_all(&meta_bytes)
                .map_err(|e| EncryptFailed(e.to_string()))?;
            nonce_file
                .sync_all()
                .map_err(|e| EncryptFailed(e.to_string()))?;
            fs::rename(&tmp_path, meta_file_path).map_err(|e| EncryptFailed(e.to_string()))?;
            sync_parent_dir(meta_file_path).map_err(|e| EncryptFailed(e.to_string()))?;
            Ok(())
        })();

        if write_result.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }

        write_result
    }

    fn open_temp_meta_file(meta_file_path: &Path) -> Result<(PathBuf, File)> {
        fs::create_dir_all(parent_dir(meta_file_path)).map_err(|e| EncryptFailed(e.to_string()))?;
        loop {
            let tmp_path = Self::temp_meta_file_path(meta_file_path);
            match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&tmp_path)
            {
                Ok(file) => return Ok((tmp_path, file)),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(EncryptFailed(e.to_string())),
            }
        }
    }

    fn temp_meta_file_path(meta_file_path: &Path) -> PathBuf {
        let mut suffix = [0u8; 8];
        getrandom::fill(&mut suffix).expect("getrandom failed");
        let suffix = hex::encode(suffix);
        let file_name = meta_file_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "meta".to_string());
        meta_file_path.with_file_name(format!("{file_name}.{suffix}.tmp"))
    }

    fn encrypt(&self, bytes: Vec<u8>, position: u32) -> Result<Vec<u8>> {
        if position == Stream::COUNTER_MAX {
            return Err(EncryptFailed(String::from("counter overflow")));
        }
        self.stream
            .encrypt(position, false, Payload::from(bytes.as_slice()))
            .map_err(|e| EncryptFailed(e.to_string()))
    }

    fn decrypt(&self, bytes: Vec<u8>, position: u32) -> Result<Vec<u8>> {
        if position == Stream::COUNTER_MAX {
            return Err(DecryptFailed(String::from("counter overflow")));
        }
        self.stream
            .decrypt(position, false, Payload::from(bytes.as_slice()))
            .map_err(|e| DecryptFailed(e.to_string()))
    }

    /// Try current nonce; if that fails, retry with the previous nonce.
    /// This covers the window during shadow-file trim where the nonce is rotated
    /// (for writing the new mmap) before the kv_map atomic swap promotes all Slices
    /// to new-nonce offsets. Readers holding old Slices in that window need the old
    /// nonce to decrypt successfully.
    fn decrypt_with_fallback(&self, bytes: Vec<u8>, position: u32) -> Result<Vec<u8>> {
        if position == Stream::COUNTER_MAX {
            return Err(DecryptFailed(String::from("counter overflow")));
        }
        match self
            .stream
            .decrypt(position, false, Payload::from(bytes.as_slice()))
            .map_err(|e| DecryptFailed(e.to_string()))
        {
            Ok(plain) => Ok(plain),
            Err(_) => {
                let prev = self.previous_nonce.ok_or_else(|| {
                    DecryptFailed("decryption failed and no previous nonce available".to_string())
                })?;
                Self::build_stream(&self.key, &prev)
                    .decrypt(position, false, Payload::from(bytes.as_slice()))
                    .map_err(|e| DecryptFailed(e.to_string()))
            }
        }
    }
}

impl Encoder for Encryptor {
    fn encode_to_bytes(
        &self,
        key: &str,
        type_token: i32,
        value: &[u8],
        position: u32,
    ) -> Result<Vec<u8>> {
        let kv_bytes = encode_kv_bytes(key, type_token, value);
        let crypt_bytes = self
            .encryptor
            .read()
            .map_err(|e| EncryptFailed(e.to_string()))?
            .encrypt(kv_bytes, position)?;
        let len = crypt_bytes.len() as u32;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(crypt_bytes.as_slice());
        Ok(data)
    }

    fn materialize_slice(
        &self,
        mmap: &crate::core::memory_map::MmapHandle,
        buf: &Buffer,
    ) -> Option<(i32, Vec<u8>)> {
        let loc = match buf {
            Buffer::Slice(loc) => loc,
            _ => return None,
        };
        let ciphertext = mmap.read(loc.byte_range());
        let kv_bytes = self.decrypt_current(ciphertext, loc.position).ok()?;
        decode_kv_type_value(&kv_bytes).ok()
    }
}

impl Decoder for Encryptor {
    fn decode_bytes(&self, data: &[u8], position: u32) -> Result<DecodeResult> {
        let (bytes_to_decode, read_len) = split_frame(data)?;
        // Every ciphertext carries a tag; a shorter frame was never written by the
        // encoder, so treat it as corruption rather than trusting what follows it.
        if bytes_to_decode.len() < TAG_LEN {
            return Err(DataInvalid);
        }
        let result = self
            .encryptor
            .read()
            .map_err(|e| DecryptFailed(e.to_string()))?
            .decrypt(bytes_to_decode.to_vec(), position)
            .and_then(|vec| Buffer::from_encoded_bytes(vec.as_slice()));
        let buffer = match result {
            Ok(data) => Some(data),
            Err(e) => {
                error!(LOG_TAG, "Failed to decode data, reason: {:?}", e);
                None
            }
        };
        Ok(DecodeResult {
            buffer,
            len: read_len,
        })
    }
}

/// Unit tests for the AES-EAX record framing: encode/decode roundtrips, the nonce
/// lifecycle (rotation, previous-nonce fallback, startup recovery) and the meta file.
#[cfg(test)]
mod tests {
    use crate::core::buffer::{Buffer, Decoder, Encoder};
    use crate::core::encrypt::{Encryptor, NONCE_LEN, Stream};
    use crate::core::test_support::TEST_KEY;
    use aead_stream::StreamPrimitive;
    use std::fs;
    use tempfile::tempdir;

    /// The ciphertext of `record` without its 4-byte big-endian length prefix.
    fn ciphertext(record: &[u8]) -> &[u8] {
        &record[size_of::<u32>()..]
    }

    #[test]
    fn encode_and_decode_roundtrip_records_across_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mmkv");
        let encryptor = Encryptor::init(&path, TEST_KEY).unwrap();
        let buffer1 = Buffer::new("key1", 1i32);
        let bytes1 = encryptor
            .encode_to_bytes("key1", buffer1.kv_type(), buffer1.kv_value(), 0)
            .unwrap();
        let decode_result1 = encryptor.decode_bytes(bytes1.as_slice(), 0).unwrap();
        assert_eq!(decode_result1.len, bytes1.len() as u32);
        assert_eq!(decode_result1.buffer, Some(buffer1.clone()));
        let buffer2 = Buffer::new("key2", 2i32);
        let bytes2 = encryptor
            .encode_to_bytes("key2", buffer2.kv_type(), buffer2.kv_value(), 1)
            .unwrap();
        let decode_result2 = encryptor.decode_bytes(bytes2.as_slice(), 1).unwrap();
        assert_eq!(decode_result2.len, bytes2.len() as u32);
        assert_eq!(decode_result2.buffer, Some(buffer2));
        assert!(
            encryptor
                .decode_bytes(bytes1.as_slice(), 1)
                .unwrap()
                .buffer
                .is_none()
        );
        let encryptor = Encryptor::init(&path, TEST_KEY).unwrap();
        let new_decode_result1 = encryptor.decode_bytes(bytes1.as_slice(), 0).unwrap();
        assert_eq!(new_decode_result1.buffer, Some(buffer1));
    }

    #[test]
    fn rotate_nonce_changes_the_ciphertext_and_the_meta_file() {
        let dir = tempdir().unwrap();
        let encryptor = Encryptor::init(&dir.path().join("mmkv"), TEST_KEY).unwrap();

        let buffer = Buffer::new("key1", 42i32);
        let ciphertext_before = encryptor
            .encode_to_bytes("key1", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();
        let nonce_before = fs::read(&encryptor.meta_file_path).unwrap();

        encryptor.rotate_nonce().unwrap();

        let nonce_after = fs::read(&encryptor.meta_file_path).unwrap();
        assert_ne!(
            nonce_before, nonce_after,
            "nonce on disk must change after rotation"
        );

        let ciphertext_after = encryptor
            .encode_to_bytes("key1", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();
        assert_ne!(
            ciphertext_before, ciphertext_after,
            "same plaintext at same counter must produce different ciphertext after rotation"
        );

        let decoded = encryptor
            .decode_bytes(ciphertext_after.as_slice(), 0)
            .unwrap();
        assert_eq!(
            decoded.buffer,
            Some(buffer),
            "new ciphertext must decode correctly"
        );

        let stale = encryptor
            .decode_bytes(ciphertext_before.as_slice(), 0)
            .unwrap();
        assert!(
            stale.buffer.is_none(),
            "old ciphertext must not decode after rotation"
        );
    }

    #[test]
    fn recover_current_nonce_restores_the_previous_generation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mmkv");
        let encryptor = Encryptor::init(&path, TEST_KEY).unwrap();

        let buffer = Buffer::new("key1", 7i32);
        let ciphertext = encryptor
            .encode_to_bytes("key1", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();
        encryptor.rotate_nonce().unwrap();

        let reopened = Encryptor::init(&path, TEST_KEY).unwrap();
        let stale = reopened.decode_bytes(ciphertext.as_slice(), 0).unwrap();
        assert!(
            stale.buffer.is_none(),
            "rotation should invalidate old ciphertext by default"
        );

        reopened
            .recover_current_nonce(ciphertext.as_slice())
            .unwrap();

        let recovered = reopened.decode_bytes(ciphertext.as_slice(), 0).unwrap();
        assert_eq!(
            recovered.buffer,
            Some(buffer),
            "startup recovery should promote the previous nonce when the file still uses it"
        );
    }

    #[test]
    fn init_rejects_a_key_that_is_not_32_hex_chars() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mmkv");
        assert!(Encryptor::init(&path, "not-hex").is_err());
        assert!(Encryptor::init(&path, "88C51C53").is_err());
        assert!(!dir.path().join("mmkv.meta").exists());
    }

    #[test]
    fn decode_rejects_malformed_frames() {
        use crate::Error::DataInvalid;
        let dir = tempdir().unwrap();
        let encryptor = Encryptor::init(&dir.path().join("mmkv"), TEST_KEY).unwrap();
        let malformed: [&[u8]; 4] = [
            &[],
            &[0, 1],
            // zero-length frame: shorter than the authentication tag
            &[0, 0, 0, 0],
            // declared length runs past the input
            &[0, 0, 0x03, 0xE8, 1, 2, 3, 4],
        ];
        for data in malformed {
            assert_eq!(
                encryptor.decode_bytes(data, 0).err(),
                Some(DataInvalid),
                "{data:?}"
            );
        }
        // Well-framed garbage fails authentication: skipped, not fatal.
        let garbage = [0, 0, 0, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let result = encryptor.decode_bytes(&garbage, 0).unwrap();
        assert!(result.buffer.is_none());
        assert_eq!(result.len, garbage.len() as u32);
    }

    #[test]
    fn temp_meta_file_paths_are_unique_and_stay_next_to_the_meta_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mmkv.meta");
        let first = super::StreamWrapper::temp_meta_file_path(&path);
        let second = super::StreamWrapper::temp_meta_file_path(&path);
        assert_ne!(first, second);
        assert_eq!(first.parent(), path.parent());
        assert_eq!(second.parent(), path.parent());
    }

    #[test]
    fn init_regenerates_a_meta_file_with_an_invalid_length() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mmkv");
        let encryptor = Encryptor::init(&path, TEST_KEY).unwrap();
        let meta_path = encryptor.meta_file_path.clone();
        let buffer = Buffer::new("key1", 3i32);
        let stale = encryptor
            .encode_to_bytes("key1", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();
        drop(encryptor);

        // Neither NONCE_LEN nor NONCE_LEN * 2: the meta file is unusable.
        fs::write(&meta_path, [0u8; 5]).unwrap();

        let encryptor = Encryptor::init(&path, TEST_KEY).unwrap();
        assert_eq!(fs::read(&meta_path).unwrap().len(), NONCE_LEN);
        assert!(
            encryptor
                .decode_bytes(stale.as_slice(), 0)
                .unwrap()
                .buffer
                .is_none(),
            "a regenerated nonce must not decode the previous generation"
        );
    }

    #[test]
    fn a_meta_file_with_a_previous_nonce_keeps_the_old_generation_readable() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mmkv");
        let encryptor = Encryptor::init(&path, TEST_KEY).unwrap();
        let meta_path = encryptor.meta_file_path.clone();
        let buffer = Buffer::new("key1", 11i32);
        let stale = encryptor
            .encode_to_bytes("key1", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();
        encryptor.rotate_nonce().unwrap();
        drop(encryptor);
        assert_eq!(fs::read(&meta_path).unwrap().len(), NONCE_LEN * 2);

        // A fresh instance loads both nonces, so the previous generation still decrypts.
        let reopened = Encryptor::init(&path, TEST_KEY).unwrap();
        assert!(
            reopened
                .decode_bytes(stale.as_slice(), 0)
                .unwrap()
                .buffer
                .is_none()
        );
        let plain = reopened.decrypt_current(ciphertext(&stale), 0).unwrap();
        assert_eq!(Buffer::from_encoded_bytes(&plain).unwrap(), buffer);
    }

    #[test]
    fn rotation_keeps_the_previous_generation_decryptable_through_the_fallback() {
        let dir = tempdir().unwrap();
        let encryptor = Encryptor::init(&dir.path().join("mmkv"), TEST_KEY).unwrap();
        let buffer = Buffer::new("key1", 5i32);
        let stale = encryptor
            .encode_to_bytes("key1", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();

        encryptor.rotate_nonce().unwrap();

        // The decoder only ever uses the current nonce, so the record is skipped...
        assert!(
            encryptor
                .decode_bytes(stale.as_slice(), 0)
                .unwrap()
                .buffer
                .is_none()
        );
        // ...while readers holding a Slice from before the rotation still decrypt it.
        let plain = encryptor.decrypt_current(ciphertext(&stale), 0).unwrap();
        assert_eq!(Buffer::from_encoded_bytes(&plain).unwrap(), buffer);
    }

    #[test]
    fn recover_current_nonce_is_a_no_op_when_the_current_nonce_still_decodes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mmkv");
        let encryptor = Encryptor::init(&path, TEST_KEY).unwrap();
        let buffer = Buffer::new("key1", 1i32);
        let record = encryptor
            .encode_to_bytes("key1", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();
        let meta_before = fs::read(&encryptor.meta_file_path).unwrap();

        encryptor.recover_current_nonce(record.as_slice()).unwrap();

        assert_eq!(fs::read(&encryptor.meta_file_path).unwrap(), meta_before);
        assert_eq!(
            encryptor.decode_bytes(record.as_slice(), 0).unwrap().buffer,
            Some(buffer)
        );
    }

    #[test]
    fn writing_the_meta_file_leaves_no_tmp_sibling_behind() {
        let dir = tempdir().unwrap();
        let encryptor = Encryptor::init(&dir.path().join("mmkv"), TEST_KEY).unwrap();
        encryptor.rotate_nonce().unwrap();

        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "leftover tmp files: {leftovers:?}");
    }

    /// The AEAD counter is per-record; the last counter value is reserved and must be
    /// refused rather than reused. `decode_bytes` reports the refusal the same way it
    /// reports any undecodable record: the frame is skipped, not fatal.
    #[test]
    fn the_reserved_counter_value_is_refused() {
        use crate::Error::{DecryptFailed, EncryptFailed};
        let dir = tempdir().unwrap();
        let encryptor = Encryptor::init(&dir.path().join("mmkv"), TEST_KEY).unwrap();
        let buffer = Buffer::new("key1", 1i32);
        let record = encryptor
            .encode_to_bytes("key1", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();

        let err = encryptor
            .encode_to_bytes(
                "key1",
                buffer.kv_type(),
                buffer.kv_value(),
                Stream::COUNTER_MAX,
            )
            .unwrap_err();
        assert_eq!(err, EncryptFailed("counter overflow".to_string()));

        let err = encryptor
            .decrypt_current(ciphertext(&record), Stream::COUNTER_MAX)
            .unwrap_err();
        assert_eq!(err, DecryptFailed("counter overflow".to_string()));

        let decoded = encryptor
            .decode_bytes(record.as_slice(), Stream::COUNTER_MAX)
            .unwrap();
        assert!(decoded.buffer.is_none());
        assert_eq!(decoded.len, record.len() as u32);
    }
}
