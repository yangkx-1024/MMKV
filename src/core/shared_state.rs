use crate::core::buffer::Buffer;
use crate::core::memory_map::MmapHandle;
use arc_swap::ArcSwap;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct SharedState {
    pub mmap: ArcSwap<MmapHandle>,
    pub kv_map: RwLock<HashMap<String, Buffer>>,
}

impl SharedState {
    pub fn new(mmap: MmapHandle, kv_map: HashMap<String, Buffer>) -> Arc<Self> {
        Arc::new(SharedState {
            mmap: ArcSwap::new(Arc::new(mmap)),
            kv_map: RwLock::new(kv_map),
        })
    }
}

pub type SharedKvMap = Arc<SharedState>;
