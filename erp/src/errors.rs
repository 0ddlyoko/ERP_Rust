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

/// Raised when methods call each other without ever coming back.
///
/// Dispatch goes through generated code, so the depth is known at every hop and the cycle can be
/// reported instead of taking the process down with a stack overflow.
#[derive(Debug, Clone, Error)]
#[error(
    "Maximum call depth ({limit}) reached — these methods are calling each other without \
     returning:\n  {}",
    .path.join("\n  ")
)]
pub struct MaximumCallDepth {
    pub limit: usize,
    /// The innermost calls, in the order they were made.
    pub path: Vec<String>,
}

/// Raised when records a caller names do not exist: never created, or deleted since.
#[derive(Debug, Clone, Error)]
#[error(
    "{model_name} #{} does not exist, or was deleted",
    .ids.iter().map(u32::to_string).collect::<Vec<_>>().join(", #")
)]
pub struct MissingRecords {
    pub model_name: String,
    pub ids: Vec<u32>,
}
