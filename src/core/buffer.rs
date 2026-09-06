use std::mem::size_of;
use std::sync::Arc;
use std::{f32, f64, str, vec};

use crate::Error::{DataInvalid, DecodeFailed, KeyNotFound, TypeMissMatch};
use crate::Result;
use crate::core::memory_map::MmapHandle;
use buffa::Message;
#[cfg(not(feature = "encryption"))]
use buffa::view::MessageView;

mod generated {
    #![allow(dead_code, unused_imports)]
    include!(concat!(env!("OUT_DIR"), "/__buffa.mod.rs"));
}
use generated::KV;

/// CRC-mode Slice location: points to the value bytes in the mmap.
#[cfg(not(feature = "encryption"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceLoc {
    pub type_token: i32,
    pub value_offset: usize,
    pub value_len: usize,
}

#[cfg(not(feature = "encryption"))]
impl SliceLoc {
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.value_offset..self.value_offset + self.value_len
    }

    /// Build a CRC-mode Slice by locating the value bytes within the mmap via KVView.
    /// `mmap_base` is the base pointer of the mmap; `record_start`/`record_len` describe
    /// the full record as written by `CrcEncoder`. Returns `None` for tombstones
    /// (empty value), malformed records, or decode failures.
    pub fn from_record(
        mmap_base: usize,
        record_start: usize,
        record_len: usize,
        type_token: i32,
        _position: u32,
    ) -> Option<Self> {
        // CRC record layout: [4-byte total_len][kv_proto_bytes][1-byte crc]
        if record_len < 5 {
            return None;
        }
        let proto_offset = record_start + 4;
        let proto_len = record_len - 5;
        // SAFETY: range validated by decode_bytes; mmap lives via Arc<RawMmap> in MmapHandle.
        let proto_bytes = unsafe {
            std::slice::from_raw_parts((mmap_base + proto_offset) as *const u8, proto_len)
        };
        let view = generated::KVView::decode_view(proto_bytes).ok()?;
        let value_len = view.value.len();
        let value_ptr = view.value.as_ptr() as usize;
        let value_offset = match value_ptr.checked_sub(mmap_base) {
            Some(off) if value_len > 0 => off,
            _ => return None,
        };
        Some(SliceLoc {
            type_token,
            value_offset,
            value_len,
        })
    }
}

/// Encryption-mode Slice location: stores the raw ciphertext range + AEAD counter.
#[cfg(feature = "encryption")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceLoc {
    pub type_token: i32,
    pub record_offset: usize,
    pub record_len: usize,
    pub position: u32,
}

#[cfg(feature = "encryption")]
impl SliceLoc {
    pub fn byte_range(&self) -> std::ops::Range<usize> {
        self.record_offset..self.record_offset + self.record_len
    }

    /// Build an encryption-mode Slice pointing at the raw ciphertext in the mmap.
    /// `record_start`/`record_len` describe the full record as written by `Encryptor`.
    pub fn from_record(
        _mmap_base: usize,
        record_start: usize,
        record_len: usize,
        type_token: i32,
        position: u32,
    ) -> Option<Self> {
        // Encryption record layout: [4-byte cipher_len][ciphertext bytes]
        if record_len < 4 {
            return None;
        }
        Some(SliceLoc {
            type_token,
            record_offset: record_start + 4,
            record_len: record_len - 4,
            position,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Buffer {
    /// In-flight write: full KV on the heap, not yet flushed to mmap.
    /// `seq` is a monotonic counter assigned at `put` time; the writer uses it
    /// to avoid overwriting a newer put with a stale Slice promotion.
    Owned { kv: Arc<KV>, seq: u64 },
    /// Committed write: points into the mmap; value decoded on read.
    Slice(SliceLoc),
}

pub trait Encoder: Send {
    fn encode_to_bytes(
        &self,
        key: &str,
        type_token: i32,
        value: &[u8],
        position: u32,
    ) -> Result<Vec<u8>>;
    /// Materialize a Slice buffer's value bytes for re-encoding during trim.
    /// Returns `(type_token, value_bytes)`, or `None` if `buf` is not a Slice.
    fn materialize_slice(&self, _mmap: &MmapHandle, _buf: &Buffer) -> Option<(i32, Vec<u8>)> {
        None
    }
}

pub struct DecodeResult {
    pub buffer: Option<Buffer>,
    pub len: u32,
}

pub trait Decoder {
    fn decode_bytes(&self, data: &[u8], position: u32) -> Result<DecodeResult>;
}

impl Buffer {
    pub(crate) fn from_kv(key: &str, t: i32, value: Vec<u8>) -> Self {
        let kv = KV {
            key: key.to_string(),
            r#type: t,
            value,
            ..Default::default()
        };
        Buffer::Owned {
            kv: Arc::new(kv),
            seq: 0,
        }
    }

    pub fn new<T: ProvideTypeToken + ToBytes>(key: &str, value: T) -> Self {
        Buffer::from_kv(key, T::type_token().token, value.to_bytes())
    }

    pub fn with_seq(self, seq: u64) -> Self {
        match self {
            Buffer::Owned { kv, .. } => Buffer::Owned { kv, seq },
            other => other,
        }
    }

    pub fn seq(&self) -> Option<u64> {
        match self {
            Buffer::Owned { seq, .. } => Some(*seq),
            Buffer::Slice(_) => None,
        }
    }

    pub fn parse<T: ProvideTypeToken + FromBytes>(&self, _mmap: &MmapHandle) -> Result<T> {
        match self {
            Buffer::Owned { kv, .. } => {
                if kv.r#type == InnerTypes::Deleted.value() {
                    return Err(KeyNotFound);
                }
                let type_token = T::type_token();
                if type_token.token != kv.r#type {
                    return Err(TypeMissMatch);
                }
                T::from_bytes(kv.value.as_slice())
            }
            #[cfg(not(feature = "encryption"))]
            Buffer::Slice(loc) => {
                if loc.type_token == InnerTypes::Deleted.value() {
                    return Err(KeyNotFound);
                }
                let type_token = T::type_token();
                if type_token.token != loc.type_token {
                    return Err(TypeMissMatch);
                }
                let value_bytes = _mmap.read(loc.byte_range());
                T::from_bytes(value_bytes)
            }
            #[cfg(feature = "encryption")]
            _ => {
                // Encryption Slice parsing is handled via Decoder in mmkv_impl::get.
                // This path should not be reached directly.
                unreachable!("encrypted Slice must be parsed via Decoder::decode_bytes")
            }
        }
    }

    pub fn deleted_buffer(key: &str) -> Self {
        Buffer::from_kv(key, InnerTypes::Deleted.value(), vec![])
    }

    pub fn from_encoded_bytes(data: &[u8]) -> Result<Self> {
        let kv = KV::decode_from_slice(data).map_err(|e| DecodeFailed(e.to_string()))?;
        Ok(Buffer::Owned {
            kv: Arc::new(kv),
            seq: 0,
        })
    }

    #[cfg(test)]
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        match self {
            Buffer::Owned { kv, .. } => kv.encode_to_vec(),
            Buffer::Slice(_) => panic!("to_bytes called on Slice variant"),
        }
    }

    #[cfg(test)]
    pub(crate) fn key(&self) -> &str {
        match self {
            Buffer::Owned { kv, .. } => kv.key.as_str(),
            Buffer::Slice(_) => panic!("key() called on Slice variant"),
        }
    }

    pub fn kv_type(&self) -> i32 {
        match self {
            Buffer::Owned { kv, .. } => kv.r#type,
            Buffer::Slice(loc) => loc.type_token,
        }
    }

    #[cfg(test)]
    pub(crate) fn kv_value(&self) -> &[u8] {
        match self {
            Buffer::Owned { kv, .. } => kv.value.as_slice(),
            Buffer::Slice(_) => panic!("kv_value() called on Slice variant"),
        }
    }

    pub fn is_deleting(&self) -> bool {
        self.kv_type() == InnerTypes::Deleted.value()
    }
}

/// Build the protobuf-encoded KV bytes from raw components.
pub fn encode_kv_bytes(key: &str, type_token: i32, value: &[u8]) -> Vec<u8> {
    KV {
        key: key.to_string(),
        r#type: type_token,
        value: value.to_vec(),
        ..Default::default()
    }
    .encode_to_vec()
}

/// Split one `[u32 big-endian len][len bytes]` frame off the front of `data`.
/// Returns the frame body and the number of bytes the whole frame occupies, or
/// `DataInvalid` when the prefix is missing or the declared length runs past `data`.
/// Must never panic: corrupted files reach this from `MMKV::new`.
pub fn split_frame(data: &[u8]) -> Result<(&[u8], u32)> {
    const PREFIX: usize = size_of::<u32>();
    let prefix: [u8; PREFIX] = data
        .get(..PREFIX)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(DataInvalid)?;
    let item_len = u32::from_be_bytes(prefix) as usize;
    let end = PREFIX.checked_add(item_len).ok_or(DataInvalid)?;
    let body = data.get(PREFIX..end).ok_or(DataInvalid)?;
    let frame_len = u32::try_from(end).map_err(|_| DataInvalid)?;
    Ok((body, frame_len))
}

/// Decode protobuf KV bytes and return `(type_token, value_bytes)`.
#[cfg_attr(not(feature = "encryption"), allow(dead_code))]
pub fn decode_kv_type_value(kv_bytes: &[u8]) -> Result<(i32, Vec<u8>)> {
    let kv = KV::decode_from_slice(kv_bytes).map_err(|e| DecodeFailed(e.to_string()))?;
    Ok((kv.r#type, kv.value))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum InnerTypes {
    I32 = 0,
    Str = 1,
    Byte = 2,
    I64 = 3,
    F32 = 4,
    F64 = 5,
    ByteArray = 6,
    I32Array = 7,
    I64Array = 8,
    F32Array = 9,
    F64Array = 10,
    Deleted = 100,
}

impl InnerTypes {
    fn value(&self) -> i32 {
        *self as i32
    }

    fn reserved(value: i32) -> bool {
        (0..=100).contains(&value)
    }
}

/// 0 ~ 100 reserved for internal usage.
pub struct TypeToken {
    pub(crate) token: i32,
}

impl TypeToken {
    /// Provide an int for type token, 0 ~ 100 reserved for internal usage.
    ///
    /// Notice: Panic when token is in 0 ~ 100
    pub fn new(token: i32) -> Self {
        if InnerTypes::reserved(token) {
            panic!("type token 0 ~ 100 reserved for internal usage");
        }
        TypeToken { token }
    }

    pub(crate) fn from_int_unchecked(token: i32) -> Self {
        TypeToken { token }
    }
}

/// See [crate::MMKV::put]
pub trait ToBytes {
    /// Serialize to bytes
    fn to_bytes(&self) -> Vec<u8>;
}

impl<T> ToBytes for &T
where
    T: ToBytes,
{
    fn to_bytes(&self) -> Vec<u8> {
        (*self).to_bytes()
    }
}

/// See [crate::MMKV::put]
pub trait ProvideTypeToken {
    /// See [TypeToken::new]
    fn type_token() -> TypeToken;
}

impl<T> ProvideTypeToken for &T
where
    T: ProvideTypeToken,
{
    fn type_token() -> TypeToken {
        T::type_token()
    }
}

impl ProvideTypeToken for &str {
    fn type_token() -> TypeToken {
        TypeToken::from_int_unchecked(InnerTypes::Str.value())
    }
}

impl ProvideTypeToken for String {
    fn type_token() -> TypeToken {
        TypeToken::from_int_unchecked(InnerTypes::Str.value())
    }
}

impl ToBytes for &str {
    fn to_bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl ToBytes for String {
    fn to_bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

impl ProvideTypeToken for bool {
    fn type_token() -> TypeToken {
        TypeToken::from_int_unchecked(InnerTypes::Byte.value())
    }
}

impl ToBytes for bool {
    fn to_bytes(&self) -> Vec<u8> {
        let out = if *self { 1u8 } else { 0u8 };
        vec![out]
    }
}

impl ProvideTypeToken for &[u8] {
    fn type_token() -> TypeToken {
        TypeToken::from_int_unchecked(InnerTypes::ByteArray.value())
    }
}

impl ProvideTypeToken for Vec<u8> {
    fn type_token() -> TypeToken {
        TypeToken::from_int_unchecked(InnerTypes::ByteArray.value())
    }
}

impl ToBytes for &[u8] {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_vec()
    }
}
macro_rules! impl_to_bytes_for_number {
    ($(($t:ty, $kv_type:expr)),+;) => {
        $(
        impl ProvideTypeToken for $t {
            fn type_token() -> TypeToken {
                TypeToken::from_int_unchecked($kv_type.value())
            }
        }
        impl ToBytes for $t {
            fn to_bytes(&self) -> Vec<u8> {
                self.to_be_bytes().to_vec()
            }
        }
        )+
    };
}

impl_to_bytes_for_number!(
    (i32, InnerTypes::I32),
    (i64, InnerTypes::I64),
    (f32, InnerTypes::F32),
    (f64, InnerTypes::F64);
);

macro_rules! impl_to_bytes_for_typed_array {
    ($(($t:ty, $kv_type:expr)),+;) => {
        $(
        impl ProvideTypeToken for &[$t] {
            fn type_token() -> TypeToken {
                TypeToken::from_int_unchecked($kv_type.value())
            }
        }
        impl ProvideTypeToken for Vec<$t> {
            fn type_token() -> TypeToken {
                TypeToken::from_int_unchecked($kv_type.value())
            }
        }
        impl ToBytes for &[$t] {
            fn to_bytes(&self) -> Vec<u8> {
                let mut vec = Vec::with_capacity(self.len() * (size_of::<$t>() / size_of::<u8>()));
                for item in *self {
                    vec.extend_from_slice(item.to_be_bytes().as_slice());
                }
                vec
            }
        }
        )+
    };
}

impl_to_bytes_for_typed_array!(
    (i32, InnerTypes::I32Array),
    (i64, InnerTypes::I64Array),
    (f32, InnerTypes::F32Array),
    (f64, InnerTypes::F64Array);
);

/// See [crate::MMKV::put]
pub trait FromBytes {
    /// Deserialize from bytes
    fn from_bytes(bytes: &[u8]) -> Result<Self>
    where
        Self: Sized;
}

impl FromBytes for String {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        String::from_utf8(bytes.to_vec()).map_err(|_| DataInvalid)
    }
}

impl FromBytes for bool {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        bytes.first().map(|byte| *byte == 1).ok_or(DataInvalid)
    }
}

impl FromBytes for Vec<u8> {
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(bytes.to_vec())
    }
}

macro_rules! impl_from_buffer_for_number {
    ($($t:ty),+;) => {
        $(
        impl FromBytes for $t {
            fn from_bytes(bytes: &[u8]) -> Result<Self> {
                const ITEM_SIZE: usize = size_of::<$t>() / size_of::<u8>();
                let array: [u8; ITEM_SIZE] = bytes
                    .get(..ITEM_SIZE)
                    .and_then(|bytes| bytes.try_into().ok())
                    .ok_or(DataInvalid)?;
                Ok(<$t>::from_be_bytes(array))
            }
        }
        )+
    };
}

impl_from_buffer_for_number!(i32, i64, f32, f64;);

macro_rules! impl_from_buffer_for_typed_array {
    ($($t:ty),+;) => {
        $(
        impl FromBytes for Vec<$t> {
            fn from_bytes(bytes: &[u8]) -> Result<Self> {
                const ITEM_SIZE: usize = size_of::<$t>() / size_of::<u8>();
                if bytes.len() % ITEM_SIZE != 0 {
                    return Err(DataInvalid);
                }
                bytes
                    .chunks_exact(ITEM_SIZE)
                    .map(|chunk| {
                        chunk
                            .try_into()
                            .map(<$t>::from_be_bytes)
                            .map_err(|_| DataInvalid)
                    })
                    .collect()
            }
        }
        )+
    };
}

impl_from_buffer_for_typed_array!(i32, i64, f32, f64;);

#[cfg(test)]
impl PartialEq for Buffer {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Buffer::Owned { kv: a, .. }, Buffer::Owned { kv: b, .. }) => a.as_ref() == b.as_ref(),
            _ => false,
        }
    }
}

/// Unit tests for the encoding primitives: `Buffer` construction and parsing,
/// `SliceLoc` location maths, `TypeToken` validation and the `FromBytes`/`ToBytes`
/// conversions. Nothing here touches a store; the only file used is an anonymous
/// temp file backing a small mmap.
#[cfg(test)]
mod tests {
    use crate::core::buffer::{Buffer, TypeMissMatch};
    use crate::core::memory_map::MmapHandle;

    fn dummy_mmap() -> MmapHandle {
        use crate::core::memory_map::MemoryMap;
        let file = tempfile::tempfile().unwrap();
        file.set_len(64).unwrap();
        MemoryMap::new(&file, 64).unwrap().to_handle()
    }

    #[test]
    fn every_value_type_roundtrips_through_encoded_bytes() {
        let mmap = dummy_mmap();

        let buffer = Buffer::new("first_key", "first_value");
        let bytes = buffer.to_bytes();
        let copy = Buffer::from_encoded_bytes(bytes.as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(&mmap), Ok("first_value".to_string()));
        assert_eq!(copy.parse::<i32>(&mmap), Err(TypeMissMatch));
        assert_eq!(copy.parse::<bool>(&mmap), Err(TypeMissMatch));

        let buffer = Buffer::new("first_key", i32::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse::<String>(&mmap), Err(TypeMissMatch));
        assert_eq!(copy.parse(&mmap), Ok(i32::MAX));
        assert_eq!(copy.parse::<bool>(&mmap), Err(TypeMissMatch));

        let buffer = Buffer::new("first_key", true);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse::<String>(&mmap), Err(TypeMissMatch));
        assert_eq!(copy.parse::<i32>(&mmap), Err(TypeMissMatch));
        assert_eq!(copy.parse(&mmap), Ok(true));

        let buffer = Buffer::new("first_key", i64::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(&mmap), Ok(i64::MAX));
        assert_eq!(copy.parse::<i32>(&mmap), Err(TypeMissMatch));

        let buffer = Buffer::new("first_key", f32::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(&mmap), Ok(f32::MAX));
        assert_eq!(copy.parse::<i32>(&mmap), Err(TypeMissMatch));

        let buffer = Buffer::new("first_key", f64::MAX);
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(&mmap), Ok(f64::MAX));
        assert_eq!(copy.parse::<f32>(&mmap), Err(TypeMissMatch));

        let byte_array = vec![u8::MIN, 2, u8::MAX];
        let buffer = Buffer::new("byte_array", byte_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(&mmap), Ok(byte_array));
        assert_eq!(copy.parse::<Vec<i32>>(&mmap), Err(TypeMissMatch));

        let i32_array = vec![i32::MIN, 2, i32::MAX];
        let buffer = Buffer::new("i32_array", i32_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(&mmap), Ok(i32_array));
        assert_eq!(copy.parse::<Vec<i64>>(&mmap), Err(TypeMissMatch));

        let i64_array = vec![i64::MIN, 2, i64::MAX];
        let buffer = Buffer::new("i64_array", i64_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(&mmap), Ok(i64_array));
        assert_eq!(copy.parse::<Vec<i32>>(&mmap), Err(TypeMissMatch));

        let f32_array = vec![f32::MIN, 2.2, f32::MAX];
        let buffer = Buffer::new("f32_array", f32_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(&mmap), Ok(f32_array));
        assert_eq!(copy.parse::<Vec<f64>>(&mmap), Err(TypeMissMatch));

        let f64_array = vec![f64::MIN, 2.2, f64::MAX];
        let buffer = Buffer::new("f64_array", f64_array.as_slice());
        let copy = Buffer::from_encoded_bytes(buffer.to_bytes().as_slice()).unwrap();
        assert_eq!(copy, buffer);
        assert_eq!(copy.parse(&mmap), Ok(f64_array));
        assert_eq!(copy.parse::<Vec<u8>>(&mmap), Err(TypeMissMatch));
    }

    #[test]
    fn from_bytes_rejects_short_input() {
        use crate::Error::DataInvalid;
        use crate::core::buffer::FromBytes;
        assert_eq!(bool::from_bytes(&[]), Err(DataInvalid));
        assert_eq!(i32::from_bytes(&[1, 2, 3]), Err(DataInvalid));
        assert_eq!(i64::from_bytes(&[]), Err(DataInvalid));
        assert_eq!(f32::from_bytes(&[0]), Err(DataInvalid));
        assert_eq!(f64::from_bytes(&[0; 7]), Err(DataInvalid));
        assert_eq!(Vec::<i32>::from_bytes(&[0; 5]), Err(DataInvalid));
        assert_eq!(Vec::<i32>::from_bytes(&[]), Ok(vec![]));
    }

    #[test]
    fn split_frame_rejects_malformed_input() {
        use crate::Error::DataInvalid;
        use crate::core::buffer::split_frame;
        let malformed: [&[u8]; 4] = [
            &[],
            &[0, 0, 0],
            &[0, 0, 0, 5, 1, 2, 3, 4],
            &[0xFF, 0xFF, 0xFF, 0xFF, 1],
        ];
        for data in malformed {
            assert_eq!(split_frame(data).err(), Some(DataInvalid), "{data:?}");
        }
        let (body, len) = split_frame(&[0, 0, 0, 2, 9, 8, 7]).unwrap();
        assert_eq!(body, &[9, 8]);
        assert_eq!(len, 6);
        let (body, len) = split_frame(&[0, 0, 0, 0, 9]).unwrap();
        assert!(body.is_empty());
        assert_eq!(len, 4);
    }

    #[test]
    fn cloning_an_owned_buffer_preserves_equality_and_value() {
        let bytes = vec![1u8, 2, 3, 4];
        let buffer = Buffer::new("shared_key", bytes.as_slice());
        let clone = buffer.clone();
        let mmap = dummy_mmap();

        assert_eq!(buffer, clone);
        assert_eq!(buffer.parse::<Vec<u8>>(&mmap), Ok(bytes.clone()));
        assert_eq!(clone.parse::<Vec<u8>>(&mmap), Ok(bytes));
    }

    #[test]
    fn from_encoded_bytes_rejects_garbage() {
        use crate::Error::DecodeFailed;
        let err = Buffer::from_encoded_bytes(&[0xFF; 16]).unwrap_err();
        assert!(matches!(err, DecodeFailed(_)), "{err:?}");
    }

    #[test]
    fn string_from_bytes_rejects_invalid_utf8() {
        use crate::Error::DataInvalid;
        use crate::core::buffer::FromBytes;
        assert_eq!(String::from_bytes(&[0xF0, 0x9F, 0x92]), Err(DataInvalid));
    }

    #[test]
    #[should_panic(expected = "type token 0 ~ 100 reserved for internal usage")]
    fn type_token_zero_is_reserved() {
        use crate::core::buffer::TypeToken;
        let _ = TypeToken::new(0);
    }

    #[test]
    #[should_panic(expected = "type token 0 ~ 100 reserved for internal usage")]
    fn type_token_hundred_is_reserved() {
        use crate::core::buffer::TypeToken;
        let _ = TypeToken::new(100);
    }

    #[test]
    fn type_token_above_the_reserved_range_is_accepted() {
        use crate::core::buffer::TypeToken;
        assert_eq!(TypeToken::new(101).token, 101);
    }

    #[test]
    fn parsing_an_owned_tombstone_reports_key_not_found() {
        use crate::Error::KeyNotFound;
        let mmap = dummy_mmap();
        assert_eq!(
            Buffer::deleted_buffer("gone").parse::<i32>(&mmap),
            Err(KeyNotFound)
        );
    }

    /// The CRC-mode `SliceLoc` points straight at the value bytes inside the record,
    /// so the location maths is only meaningful against a real encoded record.
    #[cfg(not(feature = "encryption"))]
    mod crc_slice {
        use super::dummy_mmap;
        use crate::Error::{KeyNotFound, TypeMissMatch};
        use crate::core::buffer::{Buffer, Encoder, ProvideTypeToken, SliceLoc};
        use crate::core::crc::CrcEncoder;
        use crate::core::memory_map::MemoryMap;

        #[test]
        fn from_record_rejects_records_shorter_than_the_framing() {
            assert_eq!(SliceLoc::from_record(0, 0, 4, 0, 0), None);
        }

        #[test]
        fn from_record_rejects_a_tombstone_with_an_empty_value() {
            let file = tempfile::tempfile().unwrap();
            file.set_len(128).unwrap();
            let mut mm = MemoryMap::new(&file, 128).unwrap();
            let tombstone = Buffer::deleted_buffer("gone");
            let bytes = CrcEncoder
                .encode_to_bytes("gone", tombstone.kv_type(), tombstone.kv_value(), 0)
                .unwrap();
            let record_start = mm.write_offset().unwrap();
            mm.append(&bytes).unwrap();

            assert_eq!(
                SliceLoc::from_record(
                    mm.base_ptr(),
                    record_start,
                    bytes.len(),
                    tombstone.kv_type(),
                    0
                ),
                None
            );
        }

        #[test]
        fn from_record_locates_the_value_bytes_inside_the_mmap() {
            let file = tempfile::tempfile().unwrap();
            file.set_len(128).unwrap();
            let mut mm = MemoryMap::new(&file, 128).unwrap();
            let value = vec![9u8, 8, 7, 6, 5];
            let buffer = Buffer::new("key", value.as_slice());
            let bytes = CrcEncoder
                .encode_to_bytes("key", buffer.kv_type(), buffer.kv_value(), 0)
                .unwrap();
            let record_start = mm.write_offset().unwrap();
            mm.append(&bytes).unwrap();

            let loc = SliceLoc::from_record(
                mm.base_ptr(),
                record_start,
                bytes.len(),
                buffer.kv_type(),
                0,
            )
            .expect("a record with a non-empty value must yield a location");
            assert_eq!(loc.value_len, value.len());
            assert_eq!(mm.read(loc.byte_range()).unwrap(), value.as_slice());
        }

        #[test]
        fn parsing_a_slice_honours_tombstones_and_type_tokens() {
            let mmap = dummy_mmap();
            let deleted = Buffer::Slice(SliceLoc {
                type_token: 100,
                value_offset: 8,
                value_len: 1,
            });
            assert_eq!(deleted.parse::<i32>(&mmap), Err(KeyNotFound));

            let i32_slice = Buffer::Slice(SliceLoc {
                type_token: <i32 as ProvideTypeToken>::type_token().token,
                value_offset: 8,
                value_len: 4,
            });
            assert_eq!(i32_slice.parse::<String>(&mmap), Err(TypeMissMatch));
            assert_eq!(i32_slice.parse::<i32>(&mmap), Ok(0));
        }
    }

    /// The encryption-mode `SliceLoc` only records where the ciphertext frame lives.
    #[cfg(feature = "encryption")]
    mod aead_slice {
        use crate::core::buffer::SliceLoc;

        #[test]
        fn from_record_rejects_records_shorter_than_the_length_prefix() {
            assert_eq!(SliceLoc::from_record(0, 0, 3, 7, 0), None);
        }

        #[test]
        fn from_record_strips_the_length_prefix_and_keeps_the_position() {
            let loc = SliceLoc::from_record(0, 40, 24, 7, 5).unwrap();
            assert_eq!(loc.record_offset, 44);
            assert_eq!(loc.record_len, 20);
            assert_eq!(loc.position, 5);
            assert_eq!(loc.type_token, 7);
            assert_eq!(loc.byte_range(), 44..64);
        }
    }
}
