mod cache_field;
mod cache_models;
mod cache_model;
pub mod errors;

pub use cache_field::CacheField;
pub use cache_models::CacheModels;
pub use cache_model::CacheModel;

use std::collections::HashMap;
use crate::field::FieldType;
use crate::model::MapOfFields;

#[derive(Default)]
pub struct Cache {
    cache: HashMap<String, CacheModels>,
}

impl Cache {

    pub fn is_record_present(&self, model_name: &str, id: u32) -> bool {
        if let Some(cache_models) = self.cache.get(model_name) {
            cache_models.is_record_present(id)
        } else {
            false
        }
    }

    pub fn get_cache_models(&self, model_name: &str) -> Option<&CacheModels> {
        self.cache.get(model_name)
    }

    pub fn get_cache_models_mut(&mut self, model_name: &str) -> Option<&mut CacheModels> {
        self.cache.get_mut(model_name)
    }

    pub fn get_cache_record(&self, model_name: &str, id: u32) -> Option<&CacheModel> {
        self.cache.get(model_name)?.get_model(id)
    }

    pub fn get_cache_record_mut(&mut self, model_name: &str, id: u32) -> Option<&mut CacheModel> {
        self.cache.get_mut(model_name)?.get_model_mut(id)
    }

    fn get_model_from_name_or_create(&mut self, model_name: &str) -> &mut CacheModels {
        self.cache.entry(model_name.to_string()).or_default()
    }

    pub fn get_record_field(&self, model_name: &str, id: u32, field_name: &str) -> Option<&CacheField> {
        self.cache.get(model_name)?.get_model(id)?.get_field(field_name)
    }

    pub fn get_record_field_mut(&mut self, model_name: &str, id: u32, field_name: &str) -> Option<&mut CacheField> {
        self.cache.get_mut(model_name)?.get_model_mut(id)?.get_field_mut(field_name)
    }

    pub fn insert_record_field(&mut self, model_name: &str, id: u32, field_name: &str, field_value: Option<FieldType>) {
        let cache_models = self.get_model_from_name_or_create(model_name);
        let cache_model = cache_models.get_model_or_create(id);
        let result = cache_model.insert_field(field_name, field_value);
        if let Some(result) = result {
            if result.1 {
                cache_models.add_dirty(id, vec![model_name.to_string()]);
            }
        }
    }

    pub fn insert_record_model_with_map(&mut self, model_name: &str, id: u32, fields: MapOfFields) {
        let cache_models = self.get_model_from_name_or_create(model_name);
        let cache_model = cache_models.get_model_or_create(id);
        let dirty_fields = cache_model.insert_fields(fields);
        if !dirty_fields.is_empty() {
            cache_models.add_dirty(id, dirty_fields);
        }
    }

    pub fn clear_dirty_model(&mut self, model_name: &str) {
        let cache_models = self.get_cache_models_mut(model_name);
        if let Some(cache_models) = cache_models {
            cache_models.clear_all_dirty();
        }
    }

    pub fn clear_dirty(&mut self, model_name: &str, id: u32) {
        let cache_models = self.get_cache_models_mut(model_name);
        if let Some(cache_models) = cache_models {
            cache_models.clear_dirty(id)
        }
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

#[cfg(test)]
mod tests {
    use crate::field::FieldType;
    use crate::cache::Cache;
    use std::collections::HashMap;
    use crate::model::MapOfFields;

    #[test]
    fn test_get_and_insert_field() {
        let mut cache = Cache::default();
        let mut cached_fields = HashMap::new();
        cached_fields.insert("my_field".to_string(), Some(FieldType::String("my_value".to_string())));
        cache.get_model_from_name_or_create("my_model").get_model_or_create(1).insert_fields(MapOfFields::new(cached_fields));

        // Check if retrieving the field is correct
        let cache_field = cache.get_record_field("my_model", 1, "my_field");
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &FieldType::String("my_value".to_string()));

        // Modify it
        cache.insert_record_field("my_model", 1, "my_field", Some(FieldType::String("my_value_2".to_string())));
        let cache_field = cache.get_record_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &FieldType::String("my_value_2".to_string()));

        // Clear the field
        cache.insert_record_field("my_model", 1, "my_field", None);
        let cache_field = cache.get_record_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_none());
        // Put field back
        cache.insert_record_field("my_model", 1, "my_field", Some(FieldType::String("my_value_2".to_string())));

        // Insert another model
        cache.get_model_from_name_or_create("my_model").get_model_or_create(2);
        // Inserting another model shouldn't have modified the other field
        let cache_field = cache.get_record_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &FieldType::String("my_value_2".to_string()));

        // Modifying the other model shouldn't modify the other field
        cache.insert_record_field("my_model", 2, "my_field", Some(FieldType::String("my_value_3".to_string())));
        let cache_field = cache.get_record_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &FieldType::String("my_value_2".to_string()));
        let cache_field = cache.get_record_field("my_model", 2, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &FieldType::String("my_value_3".to_string()));
    }
}
