mod cache_field;
mod cache_model;
mod cache_models;
pub mod errors;

pub use cache_field::*;
pub use cache_model::*;
pub use cache_models::*;

use crate::field::{FieldType, IdMode, SingleId};
use crate::model::{MapOfFields, ModelManager};
use std::collections::{HashMap, HashSet};

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

    /// Check if given record field is present in cache
    pub fn is_record_field(
        &self,
        model_name: &str,
        field_name: &str,
        id: &u32,
    ) -> bool {
        self.cache
            .get(model_name)
            .and_then(|cache_models| cache_models.get_model(id))
            .and_then(|cache_model| cache_model.get_field(field_name))
            .is_some()
    }

    /// Insert given record to the cache.
    ///
    /// Update dirty if update_dirty is set to true, and a modification has been done
    pub fn insert_record_field<'a, Mode: IdMode>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &'a Mode,
        field_value: Option<FieldType>,
        update_dirty: bool,
    )
    where
        &'a Mode: IntoIterator<Item = SingleId>,
    {
        let cache_models = self.get_cache_models_mut(model_name);
        for id in ids {
            cache_models.insert_field(field_name, id.get_id(), field_value.clone(), update_dirty);
        }
    }

    /// Insert given fields to the cache.
    ///
    /// Update dirty if update_dirty is set to true, and a modification has been done
    pub fn insert_record_fields(
        &mut self,
        model_name: &str,
        id: u32,
        field_values: MapOfFields,
        update_dirty: bool,
    ) {
        // TODO Allow IdMode as input
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.insert_fields(id, field_values, update_dirty);
    }

    // Dirty

    /// Retrieve all dirty fields for given record
    pub fn get_dirty_fields(&self, model_name: &str, id: &u32) -> Option<&HashSet<String>> {
        let cache_models = self.get_cache_models(model_name);
        cache_models.get_dirty(id)
    }

    /// Retrieve all dirty fields for given record
    pub fn get_dirty_map_of_fields(&self, model_name: &str, id: &u32) -> Option<MapOfFields> {
        let cache_models = self.get_cache_models(model_name);
        let cache_model = cache_models.get_model(id)?;
        let dirty_fields: Vec<&str> = cache_models.get_dirty(id)?.iter().map(|f| f.as_str()).collect();
        Some(cache_model.get_map_of_fields(&dirty_fields))
    }

    /// Clear dirty fields of given model
    pub fn clear_dirty_model(&mut self, model_name: &str) {
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.clear_all_dirty();
    }

    /// Clear dirty fields of given record
    pub fn clear_dirty(&mut self, model_name: &str, id: &u32) {
        // TODO Allow IdMode as input
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.clear_dirty(id);
    }

    // Compute

    pub fn add_to_recompute<Mode: IdMode>(&mut self, model_name: &str, field_name: &str, ids: Mode) {
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.add_to_recompute(field_name, ids);
    }

    pub fn remove_to_recompute<Mode: IdMode>(&mut self, model_name: &str, field_name: &str, ids: Mode) {
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.remove_to_recompute(field_name, ids);
    }

    pub fn is_to_recompute(&self, model_name: &str, field_name: &str, id: &u32) -> bool {
        let cache_models = self.get_cache_models(model_name);
        cache_models.is_to_recompute(field_name, id)
    }

    // Export / Import

    /// Export a copy of this cache
    pub fn export_cache(&self) -> HashMap<String, CacheModels> {
        self.cache.clone()
    }

    /// Import given cache into the current cache
    pub fn import_cache(&mut self, cache: HashMap<String, CacheModels>) {
        self.cache = cache;
    }
}
