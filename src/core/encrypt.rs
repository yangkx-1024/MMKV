use aes::Aes128;
use eax::aead::consts::U8;
use eax::aead::rand_core::RngCore;
use eax::aead::stream::{NewStream, StreamBE32, StreamPrimitive};
use eax::aead::{generic_array::GenericArray, KeyInit, OsRng, Payload};
use eax::Eax;
use std::fs;
#[cfg(unix)]
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::core::buffer::{Buffer, DecodeResult, Decoder, Encoder};
use crate::Error::{DataInvalid, DecryptFailed, EncryptFailed};
use crate::Result;

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
}

impl StreamWrapper {
    fn init(key: [u8; 16], meta_file_path: &PathBuf) -> Self {
        if meta_file_path.exists() {
            StreamWrapper::new_with_nonce(key, meta_file_path)
        } else {
            StreamWrapper::new(key, meta_file_path)
        }
    }

    fn new(key: [u8; 16], meta_file_path: &PathBuf) -> Self {
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
        let tmp_path = Self::temp_meta_file_path(meta_file_path);
        let mut meta_bytes = Vec::with_capacity(match previous_nonce {
            Some(_) => META_FILE_LEN_WITH_PREVIOUS,
            None => NONCE_LEN,
        });
        meta_bytes.extend_from_slice(current_nonce);
        if let Some(previous_nonce) = previous_nonce {
            meta_bytes.extend_from_slice(previous_nonce);
        }

        let write_result = (|| -> Result<()> {
            let mut nonce_file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp_path)
                .map_err(|e| EncryptFailed(e.to_string()))?;
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

    fn temp_meta_file_path(meta_file_path: &Path) -> PathBuf {
        let tmp_ext = match meta_file_path.extension() {
            Some(ext) => format!("{}.tmp", ext.to_string_lossy()),
            None => "tmp".to_string(),
        };
        meta_file_path.with_extension(tmp_ext)
    }

    #[cfg(unix)]
    fn sync_parent_dir(path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        File::open(parent)
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
}

impl Encoder for Encryptor {
    fn encode_to_bytes(&self, raw_buffer: &Buffer, position: u32) -> Result<Vec<u8>> {
        let bytes_to_write = raw_buffer.to_bytes();
        let crypt_bytes = self
            .encryptor
            .read()
            .map_err(|e| EncryptFailed(e.to_string()))?
            .encrypt(bytes_to_write, position)?;
        let len = crypt_bytes.len() as u32;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(crypt_bytes.as_slice());
        Ok(data)
    }

    fn before_rewrite(&self) -> Result<()> {
        self.rotate_nonce()
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

    #[test]
    fn test_crypt_buffer() {
        let path = Path::new("./mmkv");
        let encryptor = Encryptor::init(path, TEST_KEY);
        let buffer1 = Buffer::new("key1", 1);
        let bytes1 = encryptor.encode_to_bytes(&buffer1, 0).unwrap();
        let decode_result1 = encryptor.decode_bytes(bytes1.as_slice(), 0).unwrap();
        assert_eq!(decode_result1.len, bytes1.len() as u32);
        assert_eq!(decode_result1.buffer, Some(buffer1.clone()));
        let buffer2 = Buffer::new("key2", 2);
        let bytes2 = encryptor.encode_to_bytes(&buffer2, 1).unwrap();
        let decode_result2 = encryptor.decode_bytes(bytes2.as_slice(), 1).unwrap();
        assert_eq!(decode_result2.len, bytes2.len() as u32);
        assert_eq!(decode_result2.buffer, Some(buffer2));
        assert!(encryptor
            .decode_bytes(bytes1.as_slice(), 1)
            .unwrap()
            .buffer
            .is_none());
        let encryptor = Encryptor::init(path, TEST_KEY);
        let new_decode_result1 = encryptor.decode_bytes(bytes1.as_slice(), 0).unwrap();
        assert_eq!(new_decode_result1.buffer, Some(buffer1));
        let _ = fs::remove_file(&encryptor.meta_file_path);
    }

    #[test]
    fn test_rotate_nonce_changes_ciphertext() {
        let path = Path::new("./mmkv_rotate_nonce");
        let _ = fs::remove_file("./mmkv_rotate_nonce.meta");
        let encryptor = Encryptor::init(path, TEST_KEY);

        let buffer = Buffer::new("key1", 42i32);
        let ciphertext_before = encryptor.encode_to_bytes(&buffer, 0).unwrap();
        let nonce_before = fs::read(&encryptor.meta_file_path).unwrap();

        encryptor.rotate_nonce().unwrap();

        let nonce_after = fs::read(&encryptor.meta_file_path).unwrap();
        assert_ne!(
            nonce_before, nonce_after,
            "nonce on disk must change after rotation"
        );

        let ciphertext_after = encryptor.encode_to_bytes(&buffer, 0).unwrap();
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

        let _ = fs::remove_file(&encryptor.meta_file_path);
    }

    #[test]
    fn test_recover_current_nonce_restores_previous_generation() {
        let path = Path::new("./mmkv_recover_nonce");
        let _ = fs::remove_file("./mmkv_recover_nonce.meta");
        let encryptor = Encryptor::init(path, TEST_KEY);

        let buffer = Buffer::new("key1", 7i32);
        let ciphertext = encryptor.encode_to_bytes(&buffer, 0).unwrap();
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

        let _ = fs::remove_file(&reopened.meta_file_path);
    }
}
