pub mod buffer;
pub mod mmkv_impl;
mod memory_map;
mod crc;
#[cfg(feature = "encryption")]
mod crypt;
mod iter;