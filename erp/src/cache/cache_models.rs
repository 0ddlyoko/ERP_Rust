use crate::cache::{CacheModel, Dirty, Update};
use std::collections::{HashMap, HashSet};
use crate::field::FieldType;
use crate::internal::internal_model::FinalInternalModel;
use crate::model::MapOfFields;

/// Cache for a specific model type
///
/// In this struct, you can find:
/// - All cached models linked to this model type
/// - Dirty fields
/// - "To recompute" fields
#[derive(Clone)]
pub struct CacheModels {
    name: String,
    pub(crate) models: HashMap<u32, CacheModel>,
    pub(crate) dirty: HashMap<u32, HashSet<String>>,
    pub(crate) to_recompute: HashMap<String, HashSet<u32>>,
}

impl CacheModels {
    pub fn new(final_internal_model: &FinalInternalModel) -> Self {
        let mut to_recompute = HashMap::new();
        for field_name in final_internal_model.fields.keys() {
            to_recompute.insert(field_name.clone(), HashSet::new());
        }
        Self {
            name: final_internal_model.name.clone(),
            models: HashMap::default(),
            dirty: HashMap::default(),
            to_recompute,
        }
    }

    // Cache methods

    pub fn is_record_present(&self, id: &u32) -> bool {
        self.models.contains_key(id)
    }

    pub fn get_model(&self, id: &u32) -> Option<&CacheModel> {
        self.models.get(id)
    }

    pub fn get_model_mut(&mut self, id: &u32) -> Option<&mut CacheModel> {
        self.models.get_mut(id)
    }

    pub fn get_model_or_create(&mut self, id: u32) -> &mut CacheModel {
        self.models.entry(id).or_insert_with(|| CacheModel::new(id))
    }

    pub fn insert_field(
        &mut self,
        field_name: &str,
        id: u32,
        field_value: Option<FieldType>,
        update_dirty: &Dirty,
        update_if_exists: &Update,
    ) {
        // TODO Add update_compute as parameter, to remove value from computed list
        let cache_model = self.get_model_or_create(id);
        let result = cache_model.insert_field(field_name, field_value.clone(), update_if_exists);
        if matches!(update_dirty, Dirty::UpdateDirty) {
            if let Some((_cache_field, dirty)) = result {
                if dirty {
                    self.add_dirty(id, vec![field_name.to_string()]);
                }
            }
        }
    }

    pub fn insert_fields(
        &mut self,
        id: u32,
        field_values: MapOfFields,
        update_dirty: &Dirty,
        update_if_exists: &Update,
    ) {
        let cache_model = self.get_model_or_create(id);
        let dirty_fields = cache_model.insert_fields(field_values, update_if_exists);
        if matches!(update_dirty, Dirty::UpdateDirty) && !dirty_fields.is_empty() {
            self.add_dirty(id, dirty_fields);
        }
    }

    // Dirty methods

    /// Get dirty data linked to given model
    pub fn get_dirty_fields(&self) -> HashMap<u32, MapOfFields> {
        self._get_dirty_map_of_fields_from_filter(|_field_name| { true })
    }

    /// Get dirty data linked to given model and given fields
    pub fn get_dirty_fields_for_fields(&self, fields: &[&str]) -> HashMap<u32, MapOfFields> {
        self._get_dirty_map_of_fields_from_filter(|field_name| {
            !fields.contains(&field_name)
        })
    }

    /// Get all dirty fields for given records
    pub fn get_dirty_records(&self, ids: &[u32]) -> HashMap<u32, MapOfFields> {
        let mut result: HashMap<u32, MapOfFields> = HashMap::new();
        for id in ids {
            if let Some(cache_model) = self.get_model(id) {
                if let Some(dirty_fields) = self.dirty.get(id) {
                    let map: HashMap<String, Option<FieldType>> = dirty_fields.iter().filter_map(|dirty_field| {
                        let field = cache_model.get_field(dirty_field)?;
                        Some((dirty_field.clone(), field.get().cloned()))
                    }).collect();

                    if !map.is_empty() {
                        result.insert(*id, MapOfFields::new(map));
                    }
                }
            }
        }

        result
    }

    /// Get dirty data linked to given filter
    fn _get_dirty_map_of_fields_from_filter<F>(&self, filter: F) -> HashMap<u32, MapOfFields>
    where
        F: Fn(&str) -> bool,
    {
        let mut result: HashMap<u32, MapOfFields> = HashMap::new();
        for (id, dirty_fields) in &self.dirty {
            if let Some(cache_model) = self.get_model(id) {
                let map: HashMap<String, Option<FieldType>> = dirty_fields.iter().filter_map(|dirty_field| {
                    if !filter(&dirty_field.as_str()) {
                        return None;
                    }

                    let field = cache_model.get_field(dirty_field)?;
                    Some((dirty_field.clone(), field.get().cloned()))
                }).collect();


                if !map.is_empty() {
                    result.insert(*id, MapOfFields::new(map));
                }
            }
        }

        result
    }

    pub fn add_dirty(&mut self, id: u32, fields: Vec<String>) {
        self.dirty.entry(id).or_default().extend(fields);
    }

    pub fn is_dirty(&self, id: &u32) -> bool {
        self.dirty.contains_key(id)
    }

    pub fn is_field_dirty(&self, field_name: &str, id: &u32) -> bool {
        self.dirty
            .get(id)
            .map_or(false, |d| d.iter().any(|f| f == field_name))
    }

    pub fn get_dirty(&self, id: &u32) -> Option<&HashSet<String>> {
        self.dirty.get(id)
    }

    pub fn clear_all_dirty(&mut self) {
        self.dirty.clear();
    }

    pub fn clear_dirty(&mut self, ids: &[u32]) {
        self.dirty.retain(|key, _| { !ids.contains(key) });
    }

    pub fn clear_dirty_field(&mut self, field_name: &str, id: &u32) {
        if let Some(vec) = self.dirty.get_mut(id) {
            vec.retain(|f| f != field_name);
            if vec.is_empty() {
                self.dirty.remove(id);
            }
        }
    }

    // Computed methods

    pub fn add_to_recompute(&mut self, fields_name: &[&str], ids: &[u32]) {
        // TODO Pass a list of ids instead of IdMode
        for &field_name in fields_name {
            let set = self.to_recompute.get_mut(field_name).unwrap_or_else(|| panic!("Cached field {} not found for model {}", field_name, self.name));
            set.extend(ids);
        }
    }

    pub fn remove_to_recompute(&mut self, fields_name: &[&str], ids: &[u32]) {
        // TODO Pass a list of ids instead of IdMode
        for &field_name in fields_name {
            let set = self.to_recompute.get_mut(field_name).unwrap_or_else(|| panic!("Cached field {} not found for model {}", field_name, self.name));
            set.retain(|f| !ids.contains(f));
        }
    }

    pub fn is_to_recompute(&self, field_name: &str, id: &u32) -> bool {
        self.get_to_recompute(field_name).map_or(false, |set| set.contains(id))
    }

    pub fn get_to_recompute(&self, field_name: &str) -> Option<&HashSet<u32>> {
        self.to_recompute.get(field_name)
    }
}
