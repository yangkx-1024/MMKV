use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::rc::Rc;
use aes::Aes128;
use eax::{aead, Eax};
use eax::aead::{KeyInit, generic_array::GenericArray, OsRng, Payload};
use eax::aead::consts::U8;
use eax::aead::rand_core::RngCore;
use eax::aead::stream::{DecryptorBE32, EncryptorBE32, NewStream, StreamPrimitive};
use crate::core::buffer::{Buffer, Decoder, Encoder, Take};

type Aes128Eax = Eax<Aes128, U8>;

pub struct Crypt {
    encryptor: EncryptorBE32<Aes128Eax>,
    decryptor: DecryptorBE32<Aes128Eax>,
    key: [u8; 16],
    nonce: [u8; 11],
}

type Result = aead::Result<Vec<u8>>;

macro_rules! stream {
    ($k:expr, $nonce:expr) => {
        {
            let chipher = Aes128Eax::new(&$k);
            eax::aead::stream::StreamBE32::from_aead(chipher, &$nonce)
        }
    };
}

macro_rules! encryptor {
    ($k:expr, $nonce:expr) => {
        {
            stream!($k, $nonce).encryptor()
        }
    }
}

macro_rules! decryptor {
    ($k:expr, $nonce:expr) => {
        {
            stream!($k, $nonce).decryptor()
        }
    }
}

impl Crypt {
    pub fn new(key: [u8; 16]) -> Self {
        let generic_array = GenericArray::from_slice(&key);
        let mut nonce = GenericArray::default();
        OsRng.fill_bytes(&mut nonce);
        let encryptor = encryptor!(generic_array, nonce);
        let decryptor = decryptor!(generic_array, nonce);
        Self {
            encryptor,
            decryptor,
            key,
            nonce: nonce.try_into().unwrap(),
        }
    }

    pub fn new_with_nonce(key: [u8; 16], nonce: &[u8]) -> Self {
        let generic_array = GenericArray::from_slice(&key);
        let nonce = GenericArray::from_slice(nonce);
        let encryptor = encryptor!(generic_array, nonce);
        let decryptor = decryptor!(generic_array, nonce);
        Self {
            encryptor,
            decryptor,
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

    pub fn encrypt(&mut self, bytes: Vec<u8>) -> Result {
        self.encryptor.encrypt_next(Payload::from(bytes.as_slice()))
    }

    pub fn decrypt(&mut self, bytes: Vec<u8>) -> Result {
        self.decryptor.decrypt_next(Payload::from(bytes.as_slice()))
    }
}

pub struct CryptBuffer(Option<Buffer>, Rc<RefCell<Crypt>>);

impl CryptBuffer {
    pub fn new_with_buffer(buffer: Buffer, crypt: Rc<RefCell<Crypt>>) -> Self {
        CryptBuffer(Some(buffer), crypt)
    }

    pub fn new(crypt: Rc<RefCell<Crypt>>) -> Self {
        CryptBuffer(None, crypt)
    }
}

impl Encoder for CryptBuffer {
    fn encode_to_bytes(&self) -> Vec<u8> {
        let bytes_to_write = self.0.as_ref().unwrap().to_bytes();
        let crypt_bytes = self.1.borrow_mut().encrypt(bytes_to_write).unwrap();
        let len = crypt_bytes.len() as u32;
        let mut data = len.to_be_bytes().to_vec();
        data.extend_from_slice(crypt_bytes.as_slice());
        data
    }
}

impl Decoder for CryptBuffer {
    fn decode_bytes(&mut self, data: &[u8]) -> u32 {
        let item_len = u32::from_be_bytes(
            data[0..4].try_into().unwrap()
        );
        let bytes_to_decode = &data[4..(4 + item_len as usize)];
        if let Ok(data) = self.1.borrow_mut().decrypt(bytes_to_decode.to_vec()) {
            self.0 = Some(Buffer::from_encoded_bytes(data.as_slice()))
        }
        4 + item_len
    }
}

impl Take for CryptBuffer {
    fn take(self) -> Option<Buffer> {
        self.0
    }
}

impl PartialEq for CryptBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Debug for CryptBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Buffer")
            .field(&self.0).finish()
    }
}

impl Debug for Crypt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Crypt")
            .field("nonce", &hex::encode(self.nonce.to_vec()))
            .finish()
    }
}


#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::core::buffer::{Buffer, Decoder, Encoder};
    use crate::core::crypt::{Crypt, CryptBuffer};


    const TEST_KEY: &str = "88C51C536176AD8A8EE4A06F62EE897E";

    #[test]
    fn test_crypt_buffer() {
        let key = hex::decode(TEST_KEY).unwrap();
        let crypt = Rc::new(RefCell::new(Crypt::new(key.try_into().unwrap())));
        let buffer = Buffer::from_i32("key1", 1);
        let buffer = CryptBuffer::new_with_buffer(buffer, crypt.clone());
        let bytes = buffer.encode_to_bytes();
        let mut copy = CryptBuffer::new(crypt.clone());
        copy.decode_bytes(bytes.as_slice());
        assert_eq!(copy, buffer);
        let buffer = Buffer::from_i32("key2", 2);
        let buffer = CryptBuffer::new_with_buffer(buffer, crypt.clone());
        let bytes = buffer.encode_to_bytes();
        let mut copy = CryptBuffer::new(crypt.clone());
        copy.decode_bytes(bytes.as_slice());
        assert_eq!(copy, buffer);
    }
}