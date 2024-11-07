mod cache_field;
mod cache_model;
pub mod errors;

pub use cache_field::CacheField;
pub use cache_field::CacheFieldValue;
pub use cache_model::CacheMapOfFields;
pub use cache_model::CacheModel;

use std::collections::HashMap;
use crate::field::FieldType;
use crate::model::MapOfFields;

pub struct Cache {
    cache: HashMap<&'static str, HashMap<u32, CacheModel>>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn is_record_present(&self, model_name: &str, id: u32) -> bool {
        if let Some(model_cache) = self.cache.get(model_name) {
            model_cache.contains_key(&id)
        } else {
            false
        }
    }
    
    pub fn get_cache_record(&self, model_name: &str, id: u32) -> Option<&CacheModel> {
        self.cache.get(model_name)?.get(&id)
    }

    pub fn get_cache_record_mut(&mut self, model_name: &str, id: u32) -> Option<&mut CacheModel> {
        self.cache.get_mut(model_name)?.get_mut(&id)
    }

    fn get_model_from_name_or_create(&mut self, model_name: &'static str) -> &mut HashMap<u32, CacheModel> {
        self.cache.entry(model_name).or_default()
    }

    pub fn get_record_or_create(&mut self, model_name: &'static str, id: u32) -> &mut CacheModel {
        let cached_models = self.get_model_from_name_or_create(model_name);
        cached_models.entry(id).or_insert_with(|| CacheModel::new(id))
    }

    pub fn get_record_field(&self, model_name: &str, id: u32, field_name: &str) -> Option<&CacheField> {
        self.cache.get(model_name)?.get(&id)?.get_field(field_name)
    }

    pub fn get_record_field_mut(&mut self, model_name: &str, id: u32, field_name: &str) -> Option<&mut CacheField> {
        self.cache.get_mut(model_name)?.get_mut(&id)?.get_field_mut(field_name)
    }

    pub fn insert_record_field(&mut self, model_name: &'static str, id: u32, field_name: &'static str, field_value: Option<CacheFieldValue>) -> Option<&mut CacheField> {
        let cache_model = self.get_record_or_create(model_name, id);
        cache_model.insert_field(field_name, field_value)
    }

    pub fn insert_record_model_with_map(&mut self, model_name: &'static str, id: u32, fields: CacheMapOfFields) {
        let cache_model = self.get_record_or_create(model_name, id);
        cache_model.insert_fields(fields)
    }

    pub fn clear_all_dirty_of_model(&mut self, model_name: &str, id: u32) {
        let cache_model = self.get_cache_record_mut(model_name, id);
        if cache_model.is_none() {
            return;
        }
        cache_model.unwrap().clear_all_dirty();
    }

    pub fn transform_into_map_of_fields(fields: &CacheMapOfFields) -> MapOfFields {
        fields.iter().map(|(&k, v)| {
            let value = match v {
                Some(CacheFieldValue::String(value)) => Some(FieldType::String(value.clone())),
                Some(CacheFieldValue::Int(value)) => Some(FieldType::Integer(*value)),
                Some(CacheFieldValue::Float(value)) => Some(FieldType::Float(*value)),
                Some(CacheFieldValue::Bool(value)) => Some(FieldType::Bool(*value)),
                None => None,
            };

            (k, value)
        }).collect()
    }

    pub fn transform_map_to_fields_into_cache(fields: &MapOfFields) -> CacheMapOfFields {
        fields.iter().map(|(&k, v)| {
            let value = match v {
                Some(FieldType::String(value)) => Some(CacheFieldValue::String(value.clone())),
                Some(FieldType::Integer(value)) => Some(CacheFieldValue::Int(*value)),
                Some(FieldType::Float(value)) => Some(CacheFieldValue::Float(*value)),
                Some(FieldType::Bool(value)) => Some(CacheFieldValue::Bool(*value)),
                None => None,
            };

            (k, value)
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::cache::cache_field::CacheFieldValue;
    use crate::cache::Cache;
    use std::collections::HashMap;

    #[test]
    fn test_get_and_insert_field() {
        let mut cache = Cache::new();
        let mut cached_fields = HashMap::new();
        cached_fields.insert("my_field", Some(CacheFieldValue::String("my_value".to_string())));
        cache.get_record_or_create("my_model", 1).insert_fields(cached_fields);

        // Check if retrieving the field is correct
        let cache_field = cache.get_record_field("my_model", 1, "my_field");
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &CacheFieldValue::String("my_value".to_string()));

        // Modify it
        cache.insert_record_field("my_model", 1, "my_field", Some(CacheFieldValue::String("my_value_2".to_string())));
        let cache_field = cache.get_record_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &CacheFieldValue::String("my_value_2".to_string()));

        // Clear the field
        cache.insert_record_field("my_model", 1, "my_field", None);
        let cache_field = cache.get_record_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_none());
        // Put field back
        cache.insert_record_field("my_model", 1, "my_field", Some(CacheFieldValue::String("my_value_2".to_string())));

        // Insert another model
        cache.get_record_or_create("my_model", 2);
        // Inserting another model shouldn't have modified the other field
        let cache_field = cache.get_record_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &CacheFieldValue::String("my_value_2".to_string()));

        // Modifying the other model shouldn't modify the other field
        cache.insert_record_field("my_model", 2, "my_field", Some(CacheFieldValue::String("my_value_3".to_string())));
        let cache_field = cache.get_record_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &CacheFieldValue::String("my_value_2".to_string()));
        let cache_field = cache.get_record_field("my_model", 2, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &CacheFieldValue::String("my_value_3".to_string()));
    }
}
