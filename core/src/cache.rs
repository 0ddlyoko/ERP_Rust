mod cache_field;
mod cache_model;

pub use cache_field::CacheField;
pub use cache_field::CacheFieldValue;
pub use cache_model::CacheModel;

use std::collections::HashMap;

pub struct Cache {
    cache: HashMap<String, HashMap<i32, CacheModel>>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get_model(&mut self, model_name: &str, id: i32) -> Option<&CacheModel> {
        self.cache.get(model_name)?.get(&id)
    }

    pub fn get_model_mut(&mut self, model_name: &str, id: i32) -> Option<&mut CacheModel> {
        self.cache.get_mut(model_name)?.get_mut(&id)
    }

    fn get_model_from_name_or_create(&mut self, model_name: &str) -> &mut HashMap<i32, CacheModel> {
        self.cache.entry(model_name.to_string()).or_insert_with(HashMap::new)
    }

    pub fn get_model_or_create(&mut self, model_name: &str, id: i32) -> &mut CacheModel {
        let cached_models = self.get_model_from_name_or_create(model_name);
        cached_models.entry(id).or_insert_with(|| CacheModel::new(id))
    }

    pub fn get_field(&mut self, model_name: &str, id: i32, field_name: &str) -> Option<&CacheField> {
        self.cache.get(model_name)?.get(&id)?.get_field(field_name)
    }

    pub fn get_field_mut(&mut self, model_name: &str, id: i32, field_name: &str) -> Option<&mut CacheField> {
        self.cache.get_mut(model_name)?.get_mut(&id)?.get_field_mut(field_name)
    }

    pub fn insert_field(&mut self, model_name: &str, id: i32, field_name: &str, field_value: Option<CacheFieldValue>) -> Option<&mut CacheField> {
        let cache_model = self.get_model_or_create(model_name, id);
        cache_model.insert_field(field_name, field_value)
    }

    pub fn insert_model_with_map(&mut self, model_name: &str, id: i32, fields: HashMap<&str, Option<CacheFieldValue>>) {
        let cache_model = self.get_model_or_create(model_name, id);
        cache_model.insert_fields(fields)
    }
}

mod test {
    use crate::cache::cache_field::CacheFieldValue;
    use crate::cache::Cache;
    use std::collections::HashMap;

    #[test]
    fn test_get_and_insert_field() {
        let mut cache = Cache::new();
        let mut cached_fields = HashMap::new();
        cached_fields.insert("my_field", Some(CacheFieldValue::String("my_value".to_string())));
        cache.get_model_or_create("my_model", 1).insert_fields(cached_fields);

        // Check if retrieving the field is correct
        let cache_field = cache.get_field("my_model", 1, "my_field");
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &CacheFieldValue::String("my_value".to_string()));

        // Modify it
        cache.insert_field("my_model", 1, "my_field", Some(CacheFieldValue::String("my_value_2".to_string())));
        let cache_field = cache.get_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &CacheFieldValue::String("my_value_2".to_string()));

        // Clear the field
        cache.insert_field("my_model", 1, "my_field", None);
        let cache_field = cache.get_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_none());
        // Put field back
        cache.insert_field("my_model", 1, "my_field", Some(CacheFieldValue::String("my_value_2".to_string())));

        // Insert another model
        cache.get_model_or_create("my_model", 2);
        // Inserting another model shouldn't have modified the other field
        let cache_field = cache.get_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &CacheFieldValue::String("my_value_2".to_string()));

        // Modifying the other model shouldn't modify the other field
        cache.insert_field("my_model", 2, "my_field", Some(CacheFieldValue::String("my_value_3".to_string())));
        let cache_field = cache.get_field("my_model", 1, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &CacheFieldValue::String("my_value_2".to_string()));
        let cache_field = cache.get_field("my_model", 2, "my_field");
        assert!(cache_field.is_some());
        assert!(cache_field.unwrap().get().is_some());
        assert_eq!(cache_field.unwrap().get().unwrap(), &CacheFieldValue::String("my_value_3".to_string()));
    }
}
