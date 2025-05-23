use crate::field::FieldType;
use std::collections::HashMap;

// TODO Check if we still need this
//  (We need it to create a new record, but try to find a new way)
#[derive(Default, Clone)]
pub struct MapOfFields {
    pub(crate) fields: HashMap<String, Option<FieldType>>,
}

impl MapOfFields {
    pub fn new(fields: HashMap<String, Option<FieldType>>) -> MapOfFields {
        MapOfFields { fields }
    }

    pub fn get<'a, T>(&'a self, field_name: &str) -> T
    where
        &'a FieldType: Into<Option<T>>,
    {
        self.get_option(field_name).unwrap()
    }

    pub fn get_option<'a, T>(&'a self, field_name: &str) -> Option<T>
    where
        &'a FieldType: Into<Option<T>>,
    {
        let field = self.fields.get(field_name)?;
        let Some(field) = field else {
            return None;
        };
        field.into()
    }

    pub fn insert_option<T>(&mut self, field_name: &str, value: Option<T>)
    where
        T: Into<FieldType>,
    {
        if let Some(value) = value {
            self.insert(field_name, value);
        } else {
            // None
            self.insert_none(field_name);
        }
    }

    pub fn insert<T>(&mut self, field_name: &str, value: T)
    where
        T: Into<FieldType>,
    {
        self.insert_field_type(field_name, value.into());
    }

    pub fn insert_field_type(&mut self, field_name: &str, field_type: FieldType) {
        self.fields.insert(field_name.to_string(), Some(field_type));
    }

    pub fn insert_none(&mut self, field_name: &str) {
        self.fields.insert(field_name.to_string(), None);
    }

    pub fn get_keys(&self) -> Vec<&str> {
        self.fields.keys().map(|str| str.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.fields.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn contains_key(&self, field_name: &str) -> bool {
        self.fields.contains_key(field_name)
    }
}
