pub mod buffer;
pub mod config;
#[cfg(not(feature = "encryption"))]
mod crc;
#[cfg(feature = "encryption")]
mod encrypt;
pub mod io_looper;
mod iter;
mod memory_map;
pub mod mmkv_impl;
