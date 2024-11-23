use crate::cache::CacheField;
use crate::model::MapOfFields;
use std::collections::HashMap;
use crate::field::FieldType;

#[derive(Clone)]
pub struct CacheModel {
    id: u32,
    fields: HashMap<String, CacheField>,
}

impl CacheModel {
    pub fn new(id: u32) -> CacheModel {
        Self::new_with_fields(id, HashMap::new())
    }

    pub fn new_with_fields(id: u32, fields: HashMap<String, CacheField>) -> CacheModel {
        CacheModel { id, fields }
    }

    pub fn id(&self) -> u32 {
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

    /// Transform this CacheModel into a MapOfFields that contains given fields.
    pub fn get_map_of_fields(&self, fields: &[&str]) -> MapOfFields {
        let fields = fields.iter().filter_map(|&field_name| {
            let field = self.get_field(field_name)?;
            Some((field_name.to_string(), field.get().cloned()))
        }).collect();
        MapOfFields::new(fields)
    }

    /// Get the list of fields that are dirty
    pub fn get_fields_dirty(&self) -> MapOfFields {
        let fields: Vec<&str> = self.fields.iter().filter_map(|(k, v)| {
            if v.is_dirty() {
                Some(k.as_str())
            } else {
                None
            }
        }).collect();
        self.get_map_of_fields(&fields)
    }

    pub fn insert_field(&mut self, name: &str, field_value: Option<FieldType>) -> Option<&mut CacheField> {
        let entry = self.fields.entry(name.to_string());
        let cache_field = entry.or_default();
        match field_value {
            Some(field) => {
                if cache_field.get().is_none() || &field != cache_field.get().unwrap() {
                    cache_field.set_dirty();
                    cache_field.set(field);
                }
            }
            None => {
                if cache_field.is_set() {
                    cache_field.set_dirty();
                    cache_field.clear();
                }
            }
        }
        self.get_field_mut(name)
    }

    pub fn insert_fields(&mut self, fields: MapOfFields) {
        for (name, value) in fields.fields {
            self.insert_field(name.as_str(), value);
        }
    }

    pub fn clear_all_dirty(&mut self) {
        for value in self.fields.values_mut() {
            value.clear_dirty();
        }
    }

    pub fn transform_into_map_of_fields(&self) -> MapOfFields {
        let fields = self.fields.iter().map(|(k, v)| (k.clone(), v.get().cloned())).collect();
        MapOfFields::new(fields)
    }
}

#[cfg(test)]
mod tests {
    use crate::cache::{CacheField, CacheModel};
    use crate::field::FieldType;
    use std::collections::HashMap;

    #[test]
    fn test_access_valid_fields() {
        let mut map: HashMap<String, CacheField> = HashMap::new();
        map.insert("test".to_string(), CacheField::default());
        map.insert("test2".to_string(), CacheField::default());
        map.insert("test3".to_string(), CacheField::default());
        map.insert("test4".to_string(), CacheField::default());

        let mut model = CacheModel::new_with_fields(1, map);
        let test_option = model.get_field_mut("test");
        assert!(test_option.is_some());

        let test_field = test_option.unwrap();
        let result = test_field.get();
        assert!(result.is_none());

        test_field.set(FieldType::String("test".to_string()));

        let result = test_field.get();
        assert!(result.clone().is_some());
        assert_eq!(result.unwrap(), &FieldType::String("test".to_string()));
    }

    #[test]
    fn test_access_invalid_field_should_not_panic() {
        let mut map: HashMap<String, CacheField> = HashMap::new();
        map.insert("test".to_string(), CacheField::default());
        let model = CacheModel::new_with_fields(1, map);

        // Accessing to an invalid field should throw an error
        let field = model.get_field("test2");
        assert!(field.is_none());
    }
}
