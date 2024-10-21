use std::collections::HashMap;
use crate::cache::cache_field::{CacheField, CacheFieldValue};

pub struct CacheModel {
    id: i32,
    fields: HashMap<String, CacheField>,
}

impl CacheModel {
    pub fn new(id: i32) -> CacheModel {
        Self::new_with_fields(id, HashMap::new())
    }

    pub fn new_with_fields(id: i32, fields: HashMap<String, CacheField>) -> CacheModel {
        CacheModel { id, fields }
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    /// Return given field from this cached model.
    ///
    /// We assume the field name given to this method exists, as giving an invalid name or a name
    /// that does not belong to this model is invalid.
    pub fn get_field(&self, name: &str) -> Option<&CacheField> {
        self.fields.get(name)
    }

    /// Return given field from this cached model.
    ///
    /// We assume the field name given to this method exists, as giving an invalid name or a name
    /// that does not belong to this model is invalid.
    pub fn get_field_mut(&mut self, name: &str) -> Option<&mut CacheField> {
        self.fields.get_mut(name)
    }

    pub fn insert_field(&mut self, name: &str, field_value: Option<CacheFieldValue>) -> Option<&mut CacheField> {
        let cache_field = self.fields.entry(name.to_string()).or_insert_with(CacheField::new);
        match field_value {
            Some(field) => {
                cache_field.set(field);
            }
            None => {
                cache_field.clear();
            }
        }
        self.get_field_mut(name)
    }

    pub fn insert_fields(&mut self, fields: HashMap<&str, Option<CacheFieldValue>>) {
        for (name, value) in fields {
            self.insert_field(name, value);
        }
    }
}

mod test {
    use std::collections::HashMap;
    use crate::cache::cache_field::CacheFieldValue;
    use crate::cache::cache_model::{CacheModel, CacheField};

    #[test]
    fn test_access_valid_fields() {
        let mut map: HashMap<String, CacheField> = HashMap::new();
        map.insert("test".to_string(), CacheField::new());
        map.insert("test2".to_string(), CacheField::new());
        map.insert("test3".to_string(), CacheField::new());
        map.insert("test4".to_string(), CacheField::new());

        let mut model = CacheModel::new_with_fields(1, map);
        let test_option = model.get_field_mut("test");
        assert!(test_option.is_some());

        let test_field = test_option.unwrap();
        let result = test_field.get();
        assert!(result.is_none());

        test_field.set(CacheFieldValue::String("test".to_string()));

        let result = test_field.get();
        assert!(result.clone().is_some());
        assert_eq!(result.unwrap(), &CacheFieldValue::String("test".to_string()));
    }

    #[test]
    fn test_access_invalid_field_should_not_panic() {
        let mut map: HashMap<String, CacheField> = HashMap::new();
        map.insert("test".to_string(), CacheField::new());
        let model = CacheModel::new_with_fields(1, map);

        // Accessing to an invalid field should throw an error
        let field = model.get_field("test2");
        assert_eq!(field.is_none(), true);
    }
}
