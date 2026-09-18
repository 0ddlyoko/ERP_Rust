use thiserror::Error;

/// Raised when a field name that did not come from the framework itself is not declared on the
/// model it is looked up on.
#[derive(Debug, Clone, Error)]
#[error("Field \"{model_name}\".\"{field_name}\" is not present in registries!")]
pub struct FieldNotFound {
    pub model_name: String,
    pub field_name: String,
}
