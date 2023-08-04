use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
pub struct SharedData {
    pub data_map: Arc<RwLock<HashMap<String, String>>>,
}

impl Default for SharedData {
    fn default() -> Self {
        SharedData {
            data_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
