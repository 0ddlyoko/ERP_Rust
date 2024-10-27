mod model_manager;

pub use model_manager::ModelManager;

use std::collections::HashMap;
use crate::field::{FieldDescriptor, FieldType};

pub type MapOfFields = HashMap<&'static str, Option<FieldType>>;

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
    fn get_model_name() -> &'static str where Self: Sized;
    fn get_model_descriptor() -> ModelDescriptor where Self: Sized;

    /// Returns the id of the current record
    fn get_id(&self) -> u32;
    /// Returns the whole data present in this model
    fn get_data(&self) -> MapOfFields;
    /// Create an instance of this model with given list of data
    fn create_model(id: u32, data: MapOfFields) -> Self where Self: Sized;
}
