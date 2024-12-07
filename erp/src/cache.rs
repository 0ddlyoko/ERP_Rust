mod cache_field;
mod cache_model;
mod cache_models;
pub mod errors;

pub use cache_field::CacheField;
pub use cache_model::CacheModel;
pub use cache_models::CacheModels;

use crate::field::FieldType;
use crate::model::{MapOfFields, ModelManager};
use std::collections::HashMap;

pub struct Cache {
    cache: HashMap<String, CacheModels>,
}

impl Cache {
    pub fn new(model_manager: &ModelManager) -> Self {
        let mut cache = HashMap::new();
        for model_name in model_manager.get_models().keys() {
            cache.insert(model_name.clone(), CacheModels::default());
        }
        Cache { cache }
    }

    pub fn is_record_present(&self, model_name: &str, id: u32) -> bool {
        if let Some(cache_models) = self.cache.get(model_name) {
            cache_models.is_record_present(id)
        } else {
            false
        }
    }

    /// Returns CacheModels linked to given model. If CacheModels not found, panic
    pub fn get_cache_models(&self, model_name: &str) -> &CacheModels {
        self.cache
            .get(model_name)
            .unwrap_or_else(|| panic!("Model {} not found", model_name))
    }

    /// Returns CacheModels linked to given model. If CacheModels not found, panic
    pub fn get_cache_models_mut(&mut self, model_name: &str) -> &mut CacheModels {
        self.cache
            .get_mut(model_name)
            .unwrap_or_else(|| panic!("Model {} not found", model_name))
    }

    /// Returns CacheModel linked to given model & id.
    /// If CacheModels not found, panic.
    /// If id not found, return None
    pub fn get_cache_record(&self, model_name: &str, id: u32) -> Option<&CacheModel> {
        self.get_cache_models(model_name).get_model(id)
    }

    /// Returns CacheModel linked to given model & id.
    /// If CacheModels not found, panic.
    /// If id not found, return None
    pub fn get_cache_record_mut(&mut self, model_name: &str, id: u32) -> Option<&mut CacheModel> {
        self.cache.get_mut(model_name)?.get_model_mut(id)
    }

    pub fn get_record_field(
        &self,
        model_name: &str,
        id: u32,
        field_name: &str,
    ) -> Option<&CacheField> {
        self.cache
            .get(model_name)?
            .get_model(id)?
            .get_field(field_name)
    }

    pub fn get_record_field_mut(
        &mut self,
        model_name: &str,
        id: u32,
        field_name: &str,
    ) -> Option<&mut CacheField> {
        self.cache
            .get_mut(model_name)?
            .get_model_mut(id)?
            .get_field_mut(field_name)
    }

    pub fn insert_record_field(
        &mut self,
        model_name: &str,
        id: u32,
        field_name: &str,
        field_value: Option<FieldType>,
    ) {
        let cache_models = self.get_cache_models_mut(model_name);
        let cache_model = cache_models.get_model_or_create(id);
        let result = cache_model.insert_field(field_name, field_value);
        if let Some(result) = result {
            if result.1 {
                cache_models.add_dirty(id, vec![model_name.to_string()]);
            }
        }
    }

    pub fn insert_record_model_with_map(&mut self, model_name: &str, id: u32, fields: MapOfFields) {
        let cache_models = self.get_cache_models_mut(model_name);
        let cache_model = cache_models.get_model_or_create(id);
        let dirty_fields = cache_model.insert_fields(fields);
        if !dirty_fields.is_empty() {
            cache_models.add_dirty(id, dirty_fields);
        }
    }

    pub fn clear_dirty_model(&mut self, model_name: &str) {
        self.get_cache_models_mut(model_name).clear_all_dirty();
    }

    pub fn clear_dirty(&mut self, model_name: &str, id: u32) {
        self.get_cache_models_mut(model_name).clear_dirty(id);
    }

    /// Export a copy of this cache
    pub fn export_cache(&self) -> HashMap<String, CacheModels> {
        self.cache.clone()
    }

    /// Import given cache into the current cache
    pub fn import_cache(&mut self, cache: HashMap<String, CacheModels>) {
        self.cache = cache;
    }
}
