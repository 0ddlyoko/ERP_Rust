mod cache_field;
mod cache_model;
mod cache_models;
pub mod errors;

pub use cache_field::*;
pub use cache_model::*;
pub use cache_models::*;

use crate::field::{FieldType, IdMode};
use crate::model::{MapOfFields, ModelManager};
use std::collections::HashMap;

pub struct Cache {
    cache: HashMap<String, CacheModels>,
}

impl Cache {
    pub fn new(model_manager: &ModelManager) -> Self {
        let mut cache = HashMap::new();
        for model_name in model_manager.get_models().keys() {
            cache.insert(model_name.clone(), CacheModels::new(model_name.clone()));
        }
        Cache { cache }
    }

    /// Check if a given record is present in cache. If CacheModels not found, panic
    pub fn is_record_present(&self, model_name: &str, id: &u32) -> bool {
        self.get_cache_models(model_name).is_record_present(id)
    }

    /// Returns CacheModels linked to given model. If CacheModels not found, panic
    pub fn get_cache_models(&self, model_name: &str) -> &CacheModels {
        // TODO Do not panic
        self.cache
            .get(model_name)
            .unwrap_or_else(|| panic!("Model {} not found", model_name))
    }

    /// Returns CacheModels linked to given model. If CacheModels not found, panic
    pub fn get_cache_models_mut(&mut self, model_name: &str) -> &mut CacheModels {
        // TODO Do not panic
        self.cache
            .get_mut(model_name)
            .unwrap_or_else(|| panic!("Model {} not found", model_name))
    }

    /// Get value of given field for given record
    pub fn get_field_from_cache(
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

    /// Check if given record field are present in cache, and return those who are not in cache
    pub fn get_ids_not_in_cache(
        &self,
        model_name: &str,
        field_name: &str,
        ids: &[u32],
    ) -> Vec<u32> {
        let cache_models = self.cache.get(model_name);
        if cache_models.is_none() {
            return vec![];
        }
        let cache_models = cache_models.unwrap();

        let mut result = vec![];
        for id in ids {
            let cache_model = cache_models.get_model(id);
            if let Some(cache_model) = cache_model {
                if cache_model.get_field(field_name).is_none() {
                    result.push(*id);
                }
            } else {
                result.push(*id);
            }
        }

        result
    }

    /// Check if given record field is present in cache
    pub fn is_field_in_cache(
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
    /// Update dirty if UpdateDirty is given, and a modification has been done
    pub fn insert_field_in_cache(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &[u32],
        field_value: Option<FieldType>,
        update_dirty: &Dirty,
        update_if_exists: &Update,
        update_compute: &Compute,
    )
    {
        let cache_models = self.get_cache_models_mut(model_name);
        for id in ids {
            cache_models.insert_field(field_name, *id, field_value.clone(), update_dirty, update_if_exists);
        }
        if matches!(update_compute, Compute::ResetCompute) {
            cache_models.remove_to_recompute(&[field_name], ids);
        }
    }

    /// Insert given fields to the cache.
    ///
    /// Update dirty if UpdateDirty is given, and a modification has been done
    pub fn insert_fields_in_cache(
        &mut self,
        model_name: &str,
        id: u32,
        field_values: MapOfFields,
        update_dirty: &Dirty,
        update_if_exists: &Update,
    ) {
        // TODO Allow IdMode as input
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.insert_fields(id, field_values, update_dirty, update_if_exists);
    }

    // Dirty

    /// Get dirty fields linked to given model
    pub fn get_dirty_models<F>(&self, model_name: &str, field_filter: F) -> HashMap<u32, MapOfFields>
    where
        F: Fn(&str) -> bool,
    {
        let cache_models = self.get_cache_models(model_name);
        cache_models.get_dirty_fields(field_filter)
    }

    /// Get dirty fields from given list of fields
    pub fn get_dirty_fields(&self, model_name: &str, fields: &[&str]) -> HashMap<u32, MapOfFields> {
        let cache_models = self.get_cache_models(model_name);
        cache_models.get_dirty_fields_for_fields(fields)
    }

    /// Get all dirty fields for given records
    pub fn get_dirty_records<F>(&self, model_name: &str, ids: &[u32], field_filter: F) -> HashMap<u32, MapOfFields>
    where
        F: Fn(&str) -> bool,
    {
        let cache_models = self.get_cache_models(model_name);
        cache_models.get_dirty_records(ids, field_filter)
    }

    /// Clear dirty data of given model
    pub fn clear_dirty_model(&mut self, model_name: &str) {
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.clear_all_dirty();
    }

    /// Clear dirty fields of given records
    pub fn clear_dirty_fields<Mode: IdMode>(&mut self, model_name: &str, fields: &[&str], ids: &Mode) {
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.clear_dirty_records(fields, ids.get_ids_ref())
    }

    /// Clear dirty fields of given records
    pub fn clear_dirty_records<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode) {
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.clear_dirty(ids.as_ref());
    }

    // Compute

    pub fn is_field_to_recompute(&self, model_name: &str, field_name: &str, id: &u32) -> bool {
        self.cache
            .get(model_name)
            .map_or(false, |cache_models| cache_models.is_to_recompute(field_name, id))
    }

    /// Check if given record field are present in cache, and return those who are not in cache
    pub fn get_ids_to_recompute(
        &self,
        model_name: &str,
        field_name: &str,
        ids: &[u32],
    ) -> Vec<u32> {
        let cache_models = self.cache.get(model_name);
        if cache_models.is_none() {
            return vec![];
        }
        let cache_models = cache_models.unwrap();
        let ids_to_recompute = &cache_models.get_to_recompute(field_name);
        if ids_to_recompute.is_none() {
            return vec![];
        }
        let ids_to_recompute = ids_to_recompute.unwrap();

        ids_to_recompute.iter().filter_map(|id| {
            if ids.contains(id) {
                Some(*id)
            } else {
                None
            }
        }).collect()
    }

    pub fn add_ids_to_recompute(&mut self, model_name: &str, fields_name: &[&str], ids: &[u32]) {
        // TODO Pass a list of ids instead of IdMode
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.add_to_recompute(fields_name, ids);
    }

    pub fn remove_ids_from_recompute(&mut self, model_name: &str, fields_name: &[&str], ids: &[u32]) {
        // TODO Pass a list of ids instead of IdMode
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.remove_to_recompute(fields_name, ids);
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

pub enum Dirty {
    UpdateDirty,
    NotUpdateDirty,
}

pub enum Update {
    UpdateIfExists,
    NotUpdateIfExists,
}

pub enum Compute {
    ResetCompute,
    NotResetCompute,
}
