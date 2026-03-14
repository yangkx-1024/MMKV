use crate::core::buffer::Buffer;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub type SharedKvMap = Arc<RwLock<HashMap<String, Buffer>>>;

pub fn new_shared_kv_map(kv_map: HashMap<String, Buffer>) -> SharedKvMap {
    Arc::new(RwLock::new(kv_map))
}
