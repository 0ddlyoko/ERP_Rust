use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("Model \"{model_name}\" is not present in registries!")]
pub struct ModelNotFound {
    pub model_name: String,
}

/// Raised when a call names a method the registry has nothing for.
///
/// Almost always a missing `register_methods` for the model: the implementation exists and
/// compiles, but nothing ever put it in the registry.
#[derive(Debug, Clone, Error)]
#[error(
    "Method \"{model_name}\".\"{method_name}\" has no implementation registered. Did the plugin \
     call register_methods for this model?"
)]
pub struct MethodNotRegistered {
    pub model_name: String,
    pub method_name: String,
}

pub use erp_internal_types::FieldNotFound;
