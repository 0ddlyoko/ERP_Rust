mod errors;
mod map_of_fields;
mod model_descriptor;
mod model_manager;

pub use errors::*;
pub use map_of_fields::*;
pub use model_descriptor::*;
pub use model_manager::*;
use std::error::Error;

use crate::environment::Environment;

pub trait BaseModel {
    fn get_model_name() -> &'static str;
}

pub trait Model: SimplifiedModel {
    type BaseModel: BaseModel + Sized;
}

pub trait SimplifiedModel {

    fn get_model_name() -> &'static str
    where
        Self: Sized + Model {
        <Self as Model>::BaseModel::get_model_name()
    }

    fn get_model_descriptor() -> ModelDescriptor
    where
        Self: Sized;

    /// Returns the id of the current record
    fn get_id(&self) -> u32;
    /// Returns the whole data present in this model
    fn get_data(&self) -> MapOfFields;
    /// Create an instance of this model with given list of data
    fn create_model(id: u32, data: MapOfFields) -> Self
    where
        Self: Sized;

    /// Call computed method
    fn call_compute_method(
        &mut self,
        field_name: &str,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error>>;
}

// impl<T: Model> SimplifiedModel for T {
//     fn get_model_name() -> &'static str
//     where
//         Self: Sized
//     {
//         T::get_model_name()
//     }
//
//     fn get_model_descriptor() -> ModelDescriptor
//     where
//         Self: Sized
//     {
//         T::get_model_descriptor()
//     }
//
//     fn get_id(&self) -> u32 {
//         T::get_id(self)
//     }
//
//     fn get_data(&self) -> MapOfFields {
//         T::get_data(self)
//     }
//
//     fn create_model(id: u32, data: MapOfFields) -> Self
//     where
//         Self: Sized
//     {
//         T::create_model(id, data)
//     }
//
//     fn call_compute_method(&mut self, field_name: &str, env: &mut Environment) -> Result<(), Box<dyn Error>> {
//         T::call_compute_method(self, field_name, env)
//     }
// }
