use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::rc::Rc;

use aes::Aes128;
use eax::aead::consts::U8;
use eax::aead::rand_core::RngCore;
use eax::aead::stream::{NewStream, StreamBE32, StreamPrimitive};
use eax::aead::{generic_array::GenericArray, KeyInit, OsRng, Payload};
use eax::Eax;

use crate::core::buffer::{Buffer, Decoder, Encoder, Take};
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

pub struct EncryptBuffer(Option<Buffer>, Rc<RefCell<Encrypt>>);

impl EncryptBuffer {
    pub fn new_with_buffer(buffer: Buffer, crypt: Rc<RefCell<Encrypt>>) -> Self {
        EncryptBuffer(Some(buffer), crypt)
    }

    pub fn new(encrypt: Rc<RefCell<Encrypt>>) -> Self {
        EncryptBuffer(None, encrypt)
    }
}

impl Encoder for EncryptBuffer {
    fn encode_to_bytes(&self) -> Result<Vec<u8>> {
        let bytes_to_write = self.0.as_ref().unwrap().to_bytes();
        let crypt_bytes = self.1.borrow_mut().encrypt(bytes_to_write)?;
        let len = crypt_bytes.len() as u32;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(crypt_bytes.as_slice());
        Ok(data)
    }
}

impl Decoder for EncryptBuffer {
    fn decode_bytes_into(&mut self, data: &[u8]) -> Result<u32> {
        let item_len = u32::from_be_bytes(data[0..4].try_into().map_err(|_| DataInvalid)?);
        let bytes_to_decode = &data[4..(4 + item_len as usize)];
        match self.1.borrow_mut().decrypt(bytes_to_decode.to_vec()) {
            Ok(data) => {
                let buffer = Buffer::from_encoded_bytes(data.as_slice())?;
                self.0 = Some(buffer)
            }
            Err(e) => error!(LOG_TAG, "Failed to decode data, reason: {:?}", e),
        }
        Ok(4 + item_len)
    }
}

impl Take for EncryptBuffer {
    fn take(self) -> Option<Buffer> {
        self.0
    }
}

impl PartialEq for EncryptBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Debug for EncryptBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Buffer").field(&self.0).finish()
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
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::core::buffer::{Buffer, Decoder, Encoder};
    use crate::core::encrypt::{Encrypt, EncryptBuffer};

    const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

    #[test]
    fn test_crypt_buffer() {
        let encryptor = Rc::new(RefCell::new(Encrypt::new(
            hex::decode(TEST_KEY).unwrap().try_into().unwrap(),
        )));
        let nonce = encryptor.borrow_mut().nonce;
        let decryptor = Rc::new(RefCell::new(Encrypt::new_with_nonce(
            hex::decode(TEST_KEY).unwrap().try_into().unwrap(),
            &nonce,
        )));
        let buffer = Buffer::from_i32("key1", 1);
        let buffer = EncryptBuffer::new_with_buffer(buffer, encryptor.clone());
        let bytes = buffer.encode_to_bytes().unwrap();
        let mut copy = EncryptBuffer::new(decryptor.clone());
        copy.decode_bytes_into(bytes.as_slice()).unwrap();
        assert_eq!(copy, buffer);
        let buffer = Buffer::from_i32("key2", 2);
        let buffer = EncryptBuffer::new_with_buffer(buffer, encryptor.clone());
        let bytes = buffer.encode_to_bytes().unwrap();
        let mut copy = EncryptBuffer::new(decryptor.clone());
        copy.decode_bytes_into(bytes.as_slice()).unwrap();
        assert_eq!(copy, buffer);
    }
}
