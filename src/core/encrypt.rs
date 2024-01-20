use aes::Aes128;
use eax::aead::consts::U8;
use eax::aead::rand_core::RngCore;
use eax::aead::stream::{NewStream, StreamBE32, StreamPrimitive};
use eax::aead::{generic_array::GenericArray, KeyInit, OsRng, Payload};
use eax::Eax;
use std::fs;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::buffer::{Buffer, DecodeResult, Decoder, Encoder};
use crate::Error::{DataInvalid, DecryptFailed, EncryptFailed};
use crate::Result;

const LOG_TAG: &str = "MMKV:Encrypt";
const NONCE_LEN: usize = 11;

type Aes128Eax = Eax<Aes128, U8>;
type Stream = StreamBE32<Aes128Eax>;

#[derive(Clone)]
pub struct Encryptor {
    meta_file_path: PathBuf,
    encryptor: Arc<StreamWrapper>,
}

#[repr(transparent)]
struct StreamWrapper(Stream);

impl Encryptor {
    pub fn init(file_path: &Path, key: &str) -> Self {
        let decoded_key = hex::decode(key).unwrap();
        let meta_file_path = Encryptor::resolve_meta_file_path(file_path);
        let encryptor =
            StreamWrapper::init(decoded_key.as_slice().try_into().unwrap(), &meta_file_path);
        Encryptor {
            meta_file_path,
            encryptor: Arc::new(encryptor),
        }
    }

    fn resolve_meta_file_path(path: &Path) -> PathBuf {
        let mut meta_ext = "meta".to_string();
        if let Some(ext) = path.extension() {
            let ext = ext.to_str().unwrap();
            meta_ext = format!("{}.meta", ext);
        }
        path.with_extension(meta_ext)
    }

    pub fn remove_file(&self) {
        let _ = fs::remove_file(&self.meta_file_path);
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
        let generic_array = GenericArray::from_slice(key.as_slice());
        let mut nonce = GenericArray::default();
        OsRng.fill_bytes(&mut nonce);
        let mut nonce_file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(meta_file_path)
            .unwrap();
        nonce_file
            .write_all(nonce.as_slice())
            .expect("failed to write nonce file");
        let cipher = Aes128Eax::new(generic_array);
        let stream = StreamBE32::from_aead(cipher, &nonce);
        StreamWrapper(stream)
    }

    fn new_with_nonce(key: [u8; 16], meta_file_path: &PathBuf) -> Self {
        let mut nonce_file = OpenOptions::new().read(true).open(&meta_file_path).unwrap();
        let mut nonce = Vec::<u8>::new();
        let error_handle = |reason: String| {
            error!(LOG_TAG, "filed to read nonce, reason: {:?}", reason);
            warn!(LOG_TAG, "delete meta file due to previous reason, which may cause mmkv drop all encrypted data");
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
        let generic_array = GenericArray::from_slice(&key);
        let nonce = GenericArray::from_slice(nonce.as_slice());
        let cipher = Aes128Eax::new(generic_array);
        let stream = StreamBE32::from_aead(cipher, nonce);
        StreamWrapper(stream)
    }

    fn encrypt(&self, bytes: Vec<u8>, position: u32) -> Result<Vec<u8>> {
        if position == Stream::COUNTER_MAX {
            return Err(EncryptFailed(String::from("counter overflow")));
        }

        let result = self
            .0
            .encrypt(position, false, Payload::from(bytes.as_slice()))
            .map_err(|e| EncryptFailed(e.to_string()))?;

        Ok(result)
    }

    fn decrypt(&self, bytes: Vec<u8>, position: u32) -> Result<Vec<u8>> {
        if position == Stream::COUNTER_MAX {
            return Err(DecryptFailed(String::from("counter overflow")));
        }

        let result = self
            .0
            .decrypt(position, false, Payload::from(bytes.as_slice()))
            .map_err(|e| DecryptFailed(e.to_string()))?;

        Ok(result)
    }
}

impl Encoder for Encryptor {
    fn encode_to_bytes(&self, raw_buffer: &Buffer, position: u32) -> Result<Vec<u8>> {
        let bytes_to_write = raw_buffer.to_bytes();
        let crypt_bytes = self.encryptor.encrypt(bytes_to_write, position)?;
        let len = crypt_bytes.len() as u32;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(crypt_bytes.as_slice());
        Ok(data)
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
    use std::path::Path;

    const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

    #[test]
    fn test_crypt_buffer() {
        let path = Path::new("./mmkv");
        let encryptor = Encryptor::init(path, TEST_KEY);
        let buffer1 = Buffer::from_i32("key1", 1);
        let bytes1 = encryptor.encode_to_bytes(&buffer1, 0).unwrap();
        let decode_result1 = encryptor.decode_bytes(bytes1.as_slice(), 0).unwrap();
        assert_eq!(decode_result1.len, bytes1.len() as u32);
        assert_eq!(decode_result1.buffer, Some(buffer1.clone()));
        let buffer2 = Buffer::from_i32("key2", 2);
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
        encryptor.remove_file();
        assert!(!encryptor.meta_file_path.exists());
    }
}
