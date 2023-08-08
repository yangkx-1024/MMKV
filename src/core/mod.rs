pub mod buffer;
mod crc;
#[cfg(feature = "encryption")]
mod encrypt;
mod iter;
mod memory_map;
pub mod mmkv_impl;
