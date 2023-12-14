use aes::Aes128;
use eax::aead::consts::U8;
use eax::aead::rand_core::RngCore;
use eax::aead::stream::{NewStream, StreamBE32, StreamPrimitive};
use eax::aead::{generic_array::GenericArray, KeyInit, OsRng, Payload};
use eax::Eax;
use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::mem::size_of;
use std::ops::Deref;
use std::rc::Rc;

use crate::core::buffer::{Buffer, DecodeResult, Decoder, Encoder};
use crate::Error::{DataInvalid, DecryptFailed, EncryptFailed};
use crate::Result;

const LOG_TAG: &str = "MMKV:Encrypt";
pub const NONCE_LEN: usize = 11;

type Aes128Eax = Eax<Aes128, U8>;
type Stream = StreamBE32<Aes128Eax>;

pub struct Encrypt {
    stream: Stream,
    position: u32,
    key: [u8; 16],
    nonce: [u8; NONCE_LEN],
}

impl Encrypt {
    pub fn new(key: [u8; 16]) -> Self {
        let generic_array = GenericArray::from_slice(&key);
        let mut nonce = GenericArray::default();
        OsRng.fill_bytes(&mut nonce);
        let cipher = Aes128Eax::new(generic_array);
        let stream = StreamBE32::from_aead(cipher, &nonce);
        Self {
            stream,
            position: Default::default(),
            key,
            nonce: nonce.try_into().unwrap(),
        }
    }

    pub fn new_with_nonce(key: [u8; 16], nonce: &[u8]) -> Self {
        let generic_array = GenericArray::from_slice(&key);
        let nonce = GenericArray::from_slice(nonce);
        let cipher = Aes128Eax::new(generic_array);
        let stream = StreamBE32::from_aead(cipher, nonce);
        Self {
            stream,
            position: Default::default(),
            key,
            nonce: nonce.deref().try_into().unwrap(),
        }
    }

    pub fn nonce(&self) -> Vec<u8> {
        self.nonce.to_vec()
    }

    pub fn key(&self) -> Vec<u8> {
        self.key.to_vec()
    }

    pub fn encrypt(&mut self, bytes: Vec<u8>) -> Result<Vec<u8>> {
        if self.position == Stream::COUNTER_MAX {
            return Err(EncryptFailed(String::from("counter overflow")));
        }

        let result = self
            .stream
            .encrypt(self.position, false, Payload::from(bytes.as_slice()))
            .map_err(|e| EncryptFailed(e.to_string()))?;

        self.position += Stream::COUNTER_INCR;
        Ok(result)
    }

    pub fn decrypt(&mut self, bytes: Vec<u8>) -> Result<Vec<u8>> {
        if self.position == Stream::COUNTER_MAX {
            return Err(DecryptFailed(String::from("counter overflow")));
        }

        let result = self
            .stream
            .decrypt(self.position, false, Payload::from(bytes.as_slice()))
            .map_err(|e| DecryptFailed(e.to_string()))?;

        self.position += Stream::COUNTER_INCR;
        Ok(result)
    }
}

pub struct EncryptEncoderDecoder(pub Rc<RefCell<Encrypt>>);

impl Encoder for EncryptEncoderDecoder {
    fn encode_to_bytes(&self, raw_buffer: &Buffer) -> Result<Vec<u8>> {
        let bytes_to_write = raw_buffer.to_bytes();
        let crypt_bytes = self.0.borrow_mut().encrypt(bytes_to_write)?;
        let len = crypt_bytes.len() as u32;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(crypt_bytes.as_slice());
        Ok(data)
    }
}

impl Decoder for EncryptEncoderDecoder {
    fn decode_bytes(&self, data: &[u8]) -> Result<DecodeResult> {
        let data_offset = size_of::<u32>();
        let item_len =
            u32::from_be_bytes(data[0..data_offset].try_into().map_err(|_| DataInvalid)?);
        let bytes_to_decode = &data[data_offset..(data_offset + item_len as usize)];
        let read_len = data_offset as u32 + item_len;
        let result = self
            .0
            .borrow_mut()
            .decrypt(bytes_to_decode.to_vec())
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

impl Debug for Encrypt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Encrypt")
            .field("nonce", &hex::encode(self.nonce))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::buffer::{Buffer, Decoder, Encoder};
    use crate::core::encrypt::{Encrypt, EncryptEncoderDecoder};
    use std::cell::RefCell;
    use std::rc::Rc;

    const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

    #[test]
    fn test_crypt_buffer() {
        let encryptor = Encrypt::new(hex::decode(TEST_KEY).unwrap().try_into().unwrap());
        let nonce = encryptor.nonce;
        let encoder = EncryptEncoderDecoder(Rc::new(RefCell::new(encryptor)));
        let decryptor =
            Encrypt::new_with_nonce(hex::decode(TEST_KEY).unwrap().try_into().unwrap(), &nonce);
        let decoder = EncryptEncoderDecoder(Rc::new(RefCell::new(decryptor)));
        let buffer = Buffer::from_i32("key1", 1);
        let bytes = encoder.encode_to_bytes(&buffer).unwrap();
        let decode_result = decoder.decode_bytes(bytes.as_slice()).unwrap();
        assert_eq!(decode_result.len, bytes.len() as u32);
        assert_eq!(decode_result.buffer, Some(buffer));
        let buffer = Buffer::from_i32("key2", 2);
        let bytes = encoder.encode_to_bytes(&buffer).unwrap();
        let decode_result = decoder.decode_bytes(bytes.as_slice()).unwrap();
        assert_eq!(decode_result.len, bytes.len() as u32);
        assert_eq!(decode_result.buffer, Some(buffer));
    }
}
