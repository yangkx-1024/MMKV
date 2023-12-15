pub mod buffer;
pub mod config;
#[cfg(not(feature = "encryption"))]
mod crc;
#[cfg(feature = "encryption")]
mod encrypt;
mod io_looper;
mod iter;
mod memory_map;
pub mod mmkv_impl;
