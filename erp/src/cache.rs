mod cache_field;
mod cache_model;
mod cache_models;
pub mod errors;

pub use cache_field::*;
pub use cache_model::*;
pub use cache_models::*;

use crate::field::{FieldType, IdMode, SingleId};
use crate::model::{MapOfFields, ModelManager};
use std::collections::HashMap;

pub struct Cache {
    cache: HashMap<String, CacheModels>,
}

impl Cache {
    pub fn new(model_manager: &ModelManager) -> Self {
        let mut cache = HashMap::new();
        for (model_name, final_internal_model) in model_manager.get_models() {
            cache.insert(model_name.clone(), CacheModels::new(final_internal_model));
        }
        Cache { cache }
    }

    /// Check if a given record is present in cache. If CacheModels not found, panic
    pub fn is_record_present(&self, model_name: &str, id: &u32) -> bool {
        self.get_cache_models(model_name).is_record_present(id)
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

    /// Get value of given field for given record
    pub fn get_record_field(
        &self,
        model_name: &str,
        field_name: &str,
        id: &u32,
    ) -> Option<&FieldType> {
        self.cache
            .get(model_name)?
            .get_model(id)?
            .get_field(field_name)
            .and_then(|f| f.get())
    }

    pub fn insert_record_field<'a, Mode: IdMode>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &'a Mode,
        field_value: Option<FieldType>,
    )
    where
        &'a Mode: IntoIterator<Item = SingleId>,
    {
        let cache_models = self.get_cache_models_mut(model_name);
        for id in ids {
            let cache_model = cache_models.get_model_or_create(id.get_id());
            let result = cache_model.insert_field(field_name, field_value.clone());
            if let Some((_cache_field, dirty)) = result {
                if dirty {
                    cache_models.add_dirty(id.get_id(), vec![field_name.to_string()]);
                }
            }
        }
    }

    pub fn insert_record_model_with_map(&mut self, model_name: &str, id: u32, fields: MapOfFields) {
        // TODO Allow IdMode as input
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

    pub fn clear_dirty(&mut self, model_name: &str, id: &u32) {
        // TODO Allow IdMode as input
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
