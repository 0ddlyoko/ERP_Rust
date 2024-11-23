use std::collections::HashMap;
use crate::cache::CacheModel;

#[derive(Default, Clone)]
pub struct CacheModels {
    models: HashMap<u32, CacheModel>,
}

impl CacheModels {
    pub fn is_record_present(&self, id: u32) -> bool {
        self.models.contains_key(&id)
    }
    
    pub fn get_model(&self, id: u32) -> Option<&CacheModel> {
        self.models.get(&id)
    }
    
    pub fn get_model_mut(&mut self, id: u32) -> Option<&mut CacheModel> {
        self.models.get_mut(&id)
    }
    
    pub fn get_model_or_create(&mut self, id: u32) -> &mut CacheModel {
        self.models.entry(id).or_insert_with(|| CacheModel::new(id))
    }
    
    
}
