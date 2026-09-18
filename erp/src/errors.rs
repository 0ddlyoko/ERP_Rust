use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error(
    "Maximum recursion depth while computing fields {model_name}.{fields_name:?}. Combined ids are: {ids:?}"
)]
pub struct MaximumRecursionDepthCompute {
    pub model_name: String,
    pub fields_name: Vec<String>,
    pub ids: Vec<u32>,
}
