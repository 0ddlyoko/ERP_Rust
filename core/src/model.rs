mod model_manager;
mod map_of_fields;

pub use map_of_fields::MapOfFields;
pub use model_manager::ModelManager;

use crate::field::FieldDescriptor;

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
