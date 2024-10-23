use crate::cache::cache_field::{CacheField, CacheFieldValue};
use crate::cache::Cache;
use crate::model::MapOfFields;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;

pub type CacheMapOfFields = HashMap<&'static str, Option<CacheFieldValue>>;

pub struct CacheModel {
    id: u32,
    fields: HashMap<&'static str, CacheField>,
}

impl CacheModel {
    pub fn new(id: u32) -> CacheModel {
        Self::new_with_fields(id, HashMap::new())
    }

    pub fn new_with_fields(id: u32, fields: HashMap<&'static str, CacheField>) -> CacheModel {
        CacheModel { id, fields }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    /// Return given field from this cached model.
    ///
    /// We assume the field name given to this method exists, as giving an invalid name or a name
    /// that does not belong to this model is invalid.
    pub fn get_field(&self, name: &'static str) -> Option<&CacheField> {
        self.fields.get(name)
    }

    /// Return given field from this cached model.
    ///
    /// We assume the field name given to this method exists, as giving an invalid name or a name
    /// that does not belong to this model is invalid.
    pub fn get_field_mut(&mut self, name: &'static str) -> Option<&mut CacheField> {
        self.fields.get_mut(name)
    }

    /// Get the list of fields that are dirty
    pub fn get_fields_dirty(&self) -> MapOfFields {
        Cache::transform_into_map_of_fields(&self.fields.iter().filter_map(|(&k, v)| {
            if v.is_dirty() {
                Some((k, v.get().cloned()))
            } else {
                None
            }
        }).collect())
    }

    pub fn insert_field(&mut self, name: &'static str, field_value: Option<CacheFieldValue>) -> Option<&mut CacheField> {
        let entry = self.fields.entry(name);
        let exists = match entry {
            Occupied(_) => true,
            Vacant(_) => false,
        };
        let cache_field = entry.or_insert_with(CacheField::new);
        match field_value {
            Some(field) => {
                if exists && &field != cache_field.get().unwrap() {
                    cache_field.set_dirty();
                    cache_field.set(field);
                }
            }
            None => {
                if exists && cache_field.is_set() {
                    cache_field.set_dirty();
                    cache_field.clear();
                }
            }
        }
        self.get_field_mut(name)
    }

    pub fn insert_fields(&mut self, fields: CacheMapOfFields) {
        for (name, value) in fields {
            self.insert_field(name, value);
        }
    }

    pub fn transform_into_map_of_fields(&self) -> MapOfFields {
        Cache::transform_into_map_of_fields(&self.fields.iter().map(|(&k, v)| (k, v.get().cloned())).collect())
    }
}

mod test {
    use crate::cache::cache_field::CacheFieldValue;
    use crate::cache::cache_model::{CacheField, CacheModel};
    use std::collections::HashMap;

    #[test]
    fn test_access_valid_fields() {
        let mut map: HashMap<&'static str, CacheField> = HashMap::new();
        map.insert("test", CacheField::new());
        map.insert("test2", CacheField::new());
        map.insert("test3", CacheField::new());
        map.insert("test4", CacheField::new());

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
        let mut map: HashMap<&'static str, CacheField> = HashMap::new();
        map.insert("test", CacheField::new());
        let model = CacheModel::new_with_fields(1, map);

        // Accessing to an invalid field should throw an error
        let field = model.get_field("test2");
        assert_eq!(field.is_none(), true);
    }
}
