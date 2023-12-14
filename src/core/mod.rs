pub mod buffer;
#[cfg(not(feature = "encryption"))]
mod crc;
#[cfg(feature = "encryption")]
mod encrypt;
mod iter;
mod memory_map;
pub mod mmkv_impl;
