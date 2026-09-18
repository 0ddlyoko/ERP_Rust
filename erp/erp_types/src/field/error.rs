use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error(
    "Field \"{model_name}\".\"{field_name}\" for record \"{id}\" is required but is empty. This should not happen"
)]
pub struct RequiredFieldEmpty {
    pub model_name: String,
    pub field_name: String,
    pub id: u32,
}
