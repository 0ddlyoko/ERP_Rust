mod map_of_fields;
mod model_descriptor;

use std::error::Error;
pub use map_of_fields::*;
pub use model_descriptor::*;
use crate::environment::ErasedEnvironment;
use crate::field::{IdMode, MultipleIds};

/// BaseModel that represent a model, but not a single model instance by itself.
///
/// If multiple plugins implement different fields for the same model, the `BaseModel` will only be
///  defined once and be used across all those models
pub trait BaseModel {
    fn _get_model_name() -> &'static str;
}

/// Common version of the model (that will be implemented for each model)
///
/// This common trait contains methods that could be applied for both SingleId & MultipleIds structs
pub trait CommonModel<Mode: IdMode> {
    type BaseModel: BaseModel;

    /// Get this model name
    fn _get_model_name() -> &'static str
    where
        Self: Sized
    {
        Self::BaseModel::_get_model_name()
    }

    /// Get a descriptor that represents this model
    ///
    /// This method should only be called at startup, to load the model
    fn get_model_descriptor() -> ModelDescriptor
    where
        Self: Sized;

    /// Get the current id of this model
    fn get_id_mode(&self) -> &Mode;

    /// Create a new instance of this model with given id
    fn create_instance(id: Mode) -> Self
    where
        Self: Sized;

    /// Call a given computed method
    ///
    /// This method will only be called with a `Model<MultipleIds>`, not with `Model<SingleId>`
    ///
    /// So, when you implement this method in `Model<SingleId>`, you can return Ok(()), as this
    ///  method will never be called on SingleId
    fn call_compute_method(
        // &self,
        field_name: &str,
        id: MultipleIds,
        env: &mut dyn ErasedEnvironment,
    ) -> Result<(), Box<dyn Error>>
    where
        Self: Sized;
}
