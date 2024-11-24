use std::collections::HashMap;
use crate::cache::CacheModel;

#[derive(Default, Clone)]
pub struct CacheModels {
    models: HashMap<u32, CacheModel>,
    dirty: HashMap<u32, Vec<String>>,
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

    // Dirty methods

    pub fn add_dirty(&mut self, id: u32, fields: Vec<String>) {
        self.dirty.entry(id).or_insert_with(Vec::new).extend(fields);
    }

    pub fn is_dirty(&self, id: u32) -> bool {
        self.dirty.contains_key(&id)
    }

    pub fn is_field_dirty(&self, id: u32, field_name: &str) -> bool {
        self.dirty.get(&id).map_or(false, |d| d.iter().any(|f| f == field_name))
    }

    pub fn get_dirty(&self, id: u32) -> Option<&Vec<String>> {
        self.dirty.get(&id)
    }
    
    pub fn clear_all_dirty(&mut self) {
        self.dirty.clear();
    }

    pub fn clear_dirty(&mut self, id: u32) {
        self.dirty.remove(&id);
    }

    pub fn clear_dirty_field(&mut self, id: u32, field_name: &str) {
        if let Some(vec) = self.dirty.get_mut(&id) {
            vec.retain(|f| f == field_name);
            if vec.is_empty() {
                self.dirty.remove(&id);
            }
        }
    }
}
