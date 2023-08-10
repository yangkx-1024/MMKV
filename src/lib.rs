//! # Rust version of MMKV.
//! ## Examples:
//! ```
//! use mmkv::MMKV;
//!
//! let temp_dir = std::env::temp_dir();
//! MMKV::initialize(temp_dir.to_str().unwrap(), #[cfg(feature = "encryption")] "88C51C536176AD8A8EE4A06F62EE897E");
//! MMKV::put_i32("key1", 1);
//! assert_eq!(MMKV::get_i32("key1"), Some(1));
//! // Not actually needed unless you intend to delete all data
//! MMKV::clear_data();
//! ```
//! For detailed API doc, see [MMKV]
pub use crate::mmkv::MMKV;

mod core;
#[cfg(target_os = "android")]
mod jni;
mod log;
mod mmkv;
