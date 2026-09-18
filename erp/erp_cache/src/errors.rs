use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("Records {ids:?} not found for model {model_name}")]
pub struct RecordsNotFoundError {
    pub(crate) model_name: String,
    pub(crate) ids: Vec<u32>,
}
