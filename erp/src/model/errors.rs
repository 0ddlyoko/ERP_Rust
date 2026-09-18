use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("Model \"{model_name}\" is not present in registries!")]
pub struct ModelNotFound {
    pub model_name: String,
}

pub use erp_internal_types::FieldNotFound;
