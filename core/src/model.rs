pub mod model_data;
pub mod model_manager;

pub use model_manager::ModelManager;

use crate::field::{FieldDescriptor, FieldType, FromType};
use std::collections::HashMap;

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
        T: FromType<&'a FieldType> {
        self.get_option(field_name).unwrap()
    }

    pub fn get_option<'a, T>(&'a self, field_name: &str) -> Option<T>
    where
        T: FromType<&'a FieldType> {
        let field = self.fields.get(field_name)?;
        let Some(field) = field else {
            return None;
        };
        T::from_type(field)
    }

    pub fn insert<T>(&mut self, field_name: &str, value: T)
    where
        FieldType: FromType<T> {
        if let Some(field_type) = FieldType::from_type(value) {
            self.insert_field_type(field_name, field_type);
        }
    }

    pub fn insert_option<'b, T>(&mut self, field_name: &str, value: Option<T>)
    where
        FieldType: FromType<T> {
        if let Some(value) = value {
            self.insert(field_name, value);
        } else {
            // None
            self.fields.insert(field_name.to_string(), None);
        }
    }

    pub fn insert_field_type(&mut self, field_name: &str, field_type: FieldType) {
        self.fields.insert(field_name.to_string(), Some(field_type));
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

pub struct ModelDescriptor {
    pub name: String,
    pub description: Option<String>,
    pub fields: Vec<FieldDescriptor>,
}

impl ModelDescriptor {
    pub fn new(name: String) -> Self {
        let description = Some(name.clone());
        ModelDescriptor {
            name,
            description,
            fields: Vec::new(),
        }
    }
}

pub trait Model {
    fn get_model_name() -> String where Self: Sized;
    fn get_model_descriptor() -> ModelDescriptor where Self: Sized;

    /// Returns the id of the current record
    fn get_id(&self) -> u32;
    /// Returns the whole data present in this model
    fn get_data(&self) -> MapOfFields;
    /// Create an instance of this model with given list of data
    fn create_model(id: u32, data: MapOfFields) -> Self where Self: Sized;
}
