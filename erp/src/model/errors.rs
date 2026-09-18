use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("Model \"{model_name}\" is not present in registries!")]
pub struct ModelNotFound {
    pub model_name: String,
}

#[derive(Debug, Clone, Error)]
#[error("Field \"{model_name}\".\"{field_name}\" is not present in registries!")]
pub struct FieldNotFound {
    pub model_name: String,
    pub field_name: String,
}
