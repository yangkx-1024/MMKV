use aes::Aes128;
use eax::Eax;
use eax::aead::consts::U8;
use eax::aead::rand_core::RngCore;
use eax::aead::stream::{NewStream, StreamBE32, StreamPrimitive};
use eax::aead::{KeyInit, OsRng, Payload, generic_array::GenericArray};
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
    Buffer, DecodeResult, Decoder, Encoder, decode_kv_type_value, encode_kv_bytes,
};

const LOG_TAG: &str = "MMKV:Encrypt";
const NONCE_LEN: usize = 11;
const META_FILE_LEN_WITH_PREVIOUS: usize = NONCE_LEN * 2;

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
    pub fn init(file_path: &Path, key: &str) -> Self {
        let decoded_key: [u8; 16] = hex::decode(key).unwrap().as_slice().try_into().unwrap();
        let meta_file_path = Encryptor::resolve_meta_file_path(file_path);
        let encryptor = StreamWrapper::init(decoded_key, &meta_file_path);
        Encryptor {
            meta_file_path,
            encryptor: Arc::new(RwLock::new(encryptor)),
        }
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
        OsRng.fill_bytes(&mut nonce);
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
    fn init(key: [u8; 16], meta_file_path: &PathBuf) -> Self {
        if meta_file_path.exists() {
            StreamWrapper::new_with_nonce(key, meta_file_path)
        } else {
            StreamWrapper::new(key, meta_file_path)
        }
    }

    fn new(key: [u8; 16], meta_file_path: &Path) -> Self {
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        Self::write_meta_file(meta_file_path, &nonce, None).expect("failed to write nonce file");
        StreamWrapper {
            stream: Self::build_stream(&key, &nonce),
            key,
            current_nonce: nonce,
            previous_nonce: None,
        }
    }

    fn new_with_nonce(key: [u8; 16], meta_file_path: &PathBuf) -> Self {
        let mut nonce_file = OpenOptions::new().read(true).open(meta_file_path).unwrap();
        let mut nonce_bytes = Vec::<u8>::new();
        let error_handle = |reason: String| {
            error!(LOG_TAG, "filed to read nonce, reason: {:?}", reason);
            warn!(
                LOG_TAG,
                "delete meta file due to previous reason, which may cause mmkv drop all encrypted data"
            );
            let _ = fs::remove_file(meta_file_path);
            StreamWrapper::new(key, meta_file_path)
        };
        match nonce_file.read_to_end(&mut nonce_bytes) {
            Ok(len) if len != NONCE_LEN && len != META_FILE_LEN_WITH_PREVIOUS => {
                return error_handle("meta file corruption".to_string());
            }
            Err(e) => return error_handle(format!("{:?}", e)),
            _ => {}
        }
        let current_nonce: [u8; NONCE_LEN] = nonce_bytes[..NONCE_LEN].try_into().unwrap();
        let previous_nonce = if nonce_bytes.len() == META_FILE_LEN_WITH_PREVIOUS {
            Some(
                nonce_bytes[NONCE_LEN..META_FILE_LEN_WITH_PREVIOUS]
                    .try_into()
                    .unwrap(),
            )
        } else {
            None
        };
        StreamWrapper {
            stream: Self::build_stream(&key, &current_nonce),
            key,
            current_nonce,
            previous_nonce,
        }
    }

    #[cfg(test)]
    fn rotate(&mut self, meta_file_path: &Path) -> Result<()> {
        let previous_nonce = self.current_nonce;
        let mut current_nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut current_nonce);
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

    fn build_stream(key: &[u8; 16], nonce: &[u8]) -> Stream {
        let cipher = Aes128Eax::new(GenericArray::from_slice(key));
        StreamBE32::from_aead(cipher, GenericArray::from_slice(nonce))
    }

    fn can_decode_first_record(&self, data: &[u8], nonce: &[u8; NONCE_LEN]) -> bool {
        let data_offset = size_of::<u32>();
        let item_len = match data
            .get(..data_offset)
            .and_then(|prefix| prefix.try_into().ok())
            .map(u32::from_be_bytes)
        {
            Some(item_len) => item_len as usize,
            None => return false,
        };
        let data_end = match data_offset.checked_add(item_len) {
            Some(data_end) if data_end <= data.len() => data_end,
            _ => return false,
        };
        let bytes_to_decode = &data[data_offset..data_end];
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
            Self::sync_parent_dir(meta_file_path)?;
            Ok(())
        })();

        if write_result.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }

        write_result
    }

    fn parent_dir(path: &Path) -> &Path {
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
    }

    fn open_temp_meta_file(meta_file_path: &Path) -> Result<(PathBuf, File)> {
        fs::create_dir_all(Self::parent_dir(meta_file_path))
            .map_err(|e| EncryptFailed(e.to_string()))?;
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
        OsRng.fill_bytes(&mut suffix);
        let suffix = hex::encode(suffix);
        let file_name = meta_file_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "meta".to_string());
        meta_file_path.with_file_name(format!("{file_name}.{suffix}.tmp"))
    }

    #[cfg(unix)]
    fn sync_parent_dir(path: &Path) -> Result<()> {
        File::open(Self::parent_dir(path))
            .and_then(|dir| dir.sync_all())
            .map_err(|e| EncryptFailed(e.to_string()))
    }

    #[cfg(not(unix))]
    fn sync_parent_dir(_: &Path) -> Result<()> {
        Ok(())
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
        let data_offset = size_of::<u32>();
        let item_len =
            u32::from_be_bytes(data[0..data_offset].try_into().map_err(|_| DataInvalid)?);
        let bytes_to_decode = &data[data_offset..(data_offset + item_len as usize)];
        let read_len = data_offset as u32 + item_len;
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

#[cfg(test)]
mod tests {
    use crate::core::buffer::{Buffer, Decoder, Encoder};
    use crate::core::encrypt::Encryptor;
    use std::fs;
    use std::path::Path;

    const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

    /// Remove the meta file and any orphaned `<meta>.*.tmp` siblings left by a
    /// crashed atomic write (the rename never completed).
    fn cleanup_meta(meta_path: &str) {
        let _ = fs::remove_file(meta_path);
        let dir = Path::new(meta_path).parent().unwrap_or(Path::new("."));
        let prefix = Path::new(meta_path)
            .file_name()
            .map(|n| format!("{}.", n.to_string_lossy()))
            .unwrap_or_default();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with(&prefix) && name.ends_with(".tmp") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    #[test]
    fn test_crypt_buffer() {
        let path = Path::new("./mmkv");
        cleanup_meta("./mmkv.meta");
        let encryptor = Encryptor::init(path, TEST_KEY);
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
        let encryptor = Encryptor::init(path, TEST_KEY);
        let new_decode_result1 = encryptor.decode_bytes(bytes1.as_slice(), 0).unwrap();
        assert_eq!(new_decode_result1.buffer, Some(buffer1));
        cleanup_meta("./mmkv.meta");
    }

    #[test]
    fn test_rotate_nonce_changes_ciphertext() {
        let path = Path::new("./mmkv_rotate_nonce");
        cleanup_meta("./mmkv_rotate_nonce.meta");
        let encryptor = Encryptor::init(path, TEST_KEY);

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

        cleanup_meta("./mmkv_rotate_nonce.meta");
    }

    #[test]
    fn test_recover_current_nonce_restores_previous_generation() {
        let path = Path::new("./mmkv_recover_nonce");
        cleanup_meta("./mmkv_recover_nonce.meta");
        let encryptor = Encryptor::init(path, TEST_KEY);

        let buffer = Buffer::new("key1", 7i32);
        let ciphertext = encryptor
            .encode_to_bytes("key1", buffer.kv_type(), buffer.kv_value(), 0)
            .unwrap();
        encryptor.rotate_nonce().unwrap();

        let reopened = Encryptor::init(path, TEST_KEY);
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

        cleanup_meta("./mmkv_recover_nonce.meta");
    }

    #[test]
    fn test_temp_meta_file_path_is_unique() {
        let path = Path::new("./mmkv_unique.meta");
        let first = super::StreamWrapper::temp_meta_file_path(path);
        let second = super::StreamWrapper::temp_meta_file_path(path);
        assert_ne!(first, second);
        assert_eq!(first.parent(), path.parent());
        assert_eq!(second.parent(), path.parent());
    }
}
