pub use crate::mmkv::MMKV;

mod core;
#[cfg(target_os = "android")]
mod jni;
mod mmkv;
