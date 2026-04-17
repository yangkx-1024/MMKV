use aes::Aes128;
use eax::Eax;
use eax::aead::consts::U8;
use eax::aead::rand_core::RngCore;
use eax::aead::stream::{NewStream, StreamBE32, StreamPrimitive};
use eax::aead::{KeyInit, OsRng, Payload, generic_array::GenericArray};
use std::fs;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::Error::{DataInvalid, DecryptFailed, EncryptFailed};
use crate::Result;
use crate::core::buffer::{Buffer, DecodeResult, Decoder, Encoder};

const LOG_TAG: &str = "MMKV:Encrypt";
const NONCE_LEN: usize = 11;

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
        let mut nonce_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(meta_file_path)
            .unwrap();
        nonce_file
            .write_all(&nonce)
            .expect("failed to write nonce file");
        StreamWrapper {
            stream: Self::build_stream(&key, &nonce),
            key,
        }
    }

    fn new_with_nonce(key: [u8; 16], meta_file_path: &PathBuf) -> Self {
        let mut nonce_file = OpenOptions::new().read(true).open(meta_file_path).unwrap();
        let mut nonce = Vec::<u8>::new();
        let error_handle = |reason: String| {
            error!(LOG_TAG, "filed to read nonce, reason: {:?}", reason);
            warn!(
                LOG_TAG,
                "delete meta file due to previous reason, which may cause mmkv drop all encrypted data"
            );
            let _ = fs::remove_file(meta_file_path);
            StreamWrapper::new(key, meta_file_path)
        };
        match nonce_file.read_to_end(&mut nonce) {
            Ok(len) if len != NONCE_LEN => {
                return error_handle("meta file corruption".to_string());
            }
            Err(e) => return error_handle(format!("{:?}", e)),
            _ => {}
        }
        StreamWrapper {
            stream: Self::build_stream(&key, &nonce),
            key,
        }
    }

    fn rotate(&mut self, meta_file_path: &Path) -> Result<()> {
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        let mut nonce_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(meta_file_path)
            .map_err(|e| EncryptFailed(e.to_string()))?;
        nonce_file
            .write_all(&nonce)
            .map_err(|e| EncryptFailed(e.to_string()))?;
        nonce_file
            .sync_all()
            .map_err(|e| EncryptFailed(e.to_string()))?;
        // Replace in-memory stream only after the new nonce is safely on disk.
        self.stream = Self::build_stream(&self.key, &nonce);
        Ok(())
    }

    fn build_stream(key: &[u8; 16], nonce: &[u8]) -> Stream {
        let cipher = Aes128Eax::new(GenericArray::from_slice(key));
        StreamBE32::from_aead(cipher, GenericArray::from_slice(nonce))
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
        assert_ne!(nonce_before, nonce_after, "nonce on disk must change after rotation");

        let ciphertext_after = encryptor.encode_to_bytes(&buffer, 0).unwrap();
        assert_ne!(
            ciphertext_before, ciphertext_after,
            "same plaintext at same counter must produce different ciphertext after rotation"
        );

        let decoded = encryptor.decode_bytes(ciphertext_after.as_slice(), 0).unwrap();
        assert_eq!(decoded.buffer, Some(buffer), "new ciphertext must decode correctly");

        let stale = encryptor.decode_bytes(ciphertext_before.as_slice(), 0).unwrap();
        assert!(stale.buffer.is_none(), "old ciphertext must not decode after rotation");

        let _ = fs::remove_file(&encryptor.meta_file_path);
    }
}
