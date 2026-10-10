use crate::CacheModels;
use erp_types::cache::{Dirty, Update};
use erp_types::field::FieldType;
use erp_types::field::IdMode;
use erp_types::model::MapOfFields;
use std::collections::HashMap;

pub struct Cache {
    pub cache: HashMap<String, CacheModels>,
}

impl Cache {
    /// Check if a given record is present in cache. If CacheModels not found, panic
    pub fn is_record_present(&self, model_name: &str, id: u32) -> bool {
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
    pub fn get_field_from_cache(
        &self,
        model_name: &str,
        field_name: &str,
        id: u32,
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
            let cache_model = cache_models.get_model(*id);
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
    pub fn is_field_in_cache(&self, model_name: &str, field_name: &str, id: u32) -> bool {
        self.cache
            .get(model_name)
            .and_then(|cache_models| cache_models.get_model(id))
            .and_then(|cache_model| cache_model.get_field(field_name))
            .is_some()
    }

    /// Insert given record to the cache.
    ///
    /// Update dirty if UpdateDirty is given, and a modification has been done
    ///
    /// Only a write settles a pending recompute. A value loaded from the database is the one stored
    /// before the change that asked for the recompute, so it leaves the recompute pending.
    ///
    /// Returns ids that have been modified
    pub fn insert_field_in_cache(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &[u32],
        field_value: Option<FieldType>,
        update_dirty: &Dirty,
        update_if_exists: &Update,
    ) -> Vec<u32> {
        let cache_models = self.get_cache_models_mut(model_name);
        let mut updated_ids = Vec::with_capacity(ids.len());
        for id in ids {
            if cache_models.insert_field(
                field_name,
                *id,
                field_value.clone(),
                update_dirty,
                update_if_exists,
            ) {
                updated_ids.push(*id);
            }
        }
        if matches!(update_dirty, Dirty::UpdateDirty) {
            cache_models.remove_to_recompute(&[field_name], &updated_ids);
        }
        updated_ids
    }

    // Dirty

    /// Get dirty fields linked to given model
    pub fn get_dirty_models<F>(
        &self,
        model_name: &str,
        field_filter: F,
    ) -> HashMap<u32, MapOfFields>
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
    pub fn get_dirty_records<F>(
        &self,
        model_name: &str,
        ids: &[u32],
        field_filter: F,
    ) -> HashMap<u32, MapOfFields>
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
    pub fn clear_dirty_fields<Mode: IdMode>(
        &mut self,
        model_name: &str,
        fields: &[&str],
        ids: &Mode,
    ) {
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.clear_dirty_records(fields, ids.get_ids_ref())
    }

    /// Clear dirty fields of given records
    pub fn clear_dirty_records<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode) {
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.clear_dirty(ids.as_ref());
    }

    /// Forget a field's value on the given records, so the next read loads it again.
    pub fn invalidate_field(&mut self, model_name: &str, field_name: &str, ids: &[u32]) {
        self.get_cache_models_mut(model_name)
            .invalidate_field(field_name, ids);
    }

    /// Forget a field's value on every record of the model, so the next read loads it again.
    pub fn invalidate_field_everywhere(&mut self, model_name: &str, field_name: &str) {
        self.get_cache_models_mut(model_name)
            .invalidate_field_everywhere(field_name);
    }

    /// Forget the given records: values, dirty flags and pending recomputations.
    pub fn remove_records<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode) {
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.remove_models(ids.as_ref());
    }

    // Compute

    pub fn is_field_to_recompute(&self, model_name: &str, field_name: &str, id: u32) -> bool {
        self.cache
            .get(model_name)
            .is_some_and(|cache_models| cache_models.is_to_recompute(field_name, id))
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

        ids_to_recompute
            .iter()
            .filter_map(|id| if ids.contains(id) { Some(*id) } else { None })
            .collect()
    }

    pub fn add_ids_to_recompute(&mut self, model_name: &str, fields_name: &[&str], ids: &[u32]) {
        let cache_models = self.get_cache_models_mut(model_name);
        cache_models.add_to_recompute(fields_name, ids);
    }

    pub fn remove_ids_from_recompute(
        &mut self,
        model_name: &str,
        fields_name: &[&str],
        ids: &[u32],
    ) {
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
