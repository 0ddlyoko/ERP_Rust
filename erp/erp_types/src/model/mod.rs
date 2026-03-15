mod map_of_fields;
mod model_descriptor;

pub use map_of_fields::*;
pub use model_descriptor::*;

/// BaseModel that represent a model, but not a single model instance by itself.
///
/// If multiple plugins implement different fields for the same model, the `BaseModel` will only be
///  defined once and be used across all those models
pub trait BaseModel {
    fn _get_model_name() -> &'static str;
}
