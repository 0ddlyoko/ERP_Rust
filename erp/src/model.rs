mod map_of_fields;
mod model_descriptor;
mod model_manager;

pub use map_of_fields::*;
pub use model_descriptor::*;
pub use model_manager::*;

pub trait Model {

    fn get_model_name() -> String where Self: Sized;
    fn get_model_descriptor() -> ModelDescriptor<Self>
    where
        Self: Model + Default,
    ;

    /// Returns the id of the current record
    fn get_id(&self) -> u32;
    /// Returns the whole data present in this model
    fn get_data(&self) -> MapOfFields;
    /// Create an instance of this model with given list of data
    fn create_model(id: u32, data: MapOfFields) -> Self where Self: Sized;
}