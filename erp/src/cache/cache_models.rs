use crate::cache::CacheModel;
use std::collections::{HashMap, HashSet};
use crate::field::{FieldType, IdMode};
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
    models: HashMap<u32, CacheModel>,
    dirty: HashMap<u32, HashSet<String>>,
    to_recompute: HashMap<String, HashSet<u32>>,
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

    pub fn insert_field(&mut self, field_name: &str, id: u32, field_value: Option<FieldType>, update_dirty: bool) {
        let cache_model = self.get_model_or_create(id);
        let result = cache_model.insert_field(field_name, field_value.clone());
        if update_dirty {
            if let Some((_cache_field, dirty)) = result {
                if dirty {
                    self.add_dirty(id, vec![field_name.to_string()]);
                }
            }
        }
    }

    pub fn insert_fields(&mut self, id: u32, field_values: MapOfFields, update_dirty: bool) {
        let cache_model = self.get_model_or_create(id);
        let dirty_fields = cache_model.insert_fields(field_values);
        if update_dirty && !dirty_fields.is_empty() {
            self.add_dirty(id, dirty_fields);
        }
    }

    // Dirty methods

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

    pub fn clear_dirty(&mut self, id: &u32) {
        self.dirty.remove(id);
    }

    pub fn clear_dirty_field(&mut self, field_name: &str, id: &u32) {
        if let Some(vec) = self.dirty.get_mut(id) {
            vec.retain(|f| f == field_name);
            if vec.is_empty() {
                self.dirty.remove(id);
            }
        }
    }

    // Computed methods

    pub fn add_to_recompute<Mode: IdMode>(&mut self, field_name: &str, ids: Mode) {
        let set = self.to_recompute.get_mut(field_name).unwrap_or_else(|| panic!("Cached field {} not found for model {}", field_name, self.name));
        set.extend(ids.get_ids_ref());
    }

    pub fn remove_to_recompute<Mode: IdMode>(&mut self, field_name: &str, ids: Mode) {
        let set = self.to_recompute.get_mut(field_name).unwrap_or_else(|| panic!("Cached field {} not found for model {}", field_name, self.name));
        let ids = ids.get_ids_ref();
        set.retain(|f| ids.contains(f));
    }

    pub fn is_to_recompute(&self, field_name: &str, id: &u32) -> bool {
        let set = &self.to_recompute.get(field_name).unwrap_or_else(|| panic!("Cached field {} not found for model {}", field_name, self.name));
        set.contains(id)
    }
}
