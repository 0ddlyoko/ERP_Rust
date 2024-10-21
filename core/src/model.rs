mod model_manager;

pub use model_manager::ModelManager;

use std::collections::HashMap;
use crate::field::{FieldDescriptor, FieldType};

pub type MapOfFields<'a> = HashMap<&'a str, Option<FieldType>>;

pub struct ModelDescriptor {
    pub name: String,
    pub description: String,
    pub fields: Vec<FieldDescriptor>,
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
