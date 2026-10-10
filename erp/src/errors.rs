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

/// A business rule refuses what was asked: the user is told why, and may act on it — a carton
/// loaded past what it holds, an invoice posted in a locked period.
#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct BusinessError {
    pub message: String,
}

impl BusinessError {
    pub fn new(message: impl Into<String>) -> Self {
        BusinessError {
            message: message.into(),
        }
    }
}

/// What the user gave does not hold — a required field left empty, a value of the wrong kind —
/// naming the field when there is one, for the client to point at it.
#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct InputError {
    pub message: String,
    pub field: Option<String>,
}

impl InputError {
    pub fn new(message: impl Into<String>) -> Self {
        InputError {
            message: message.into(),
            field: None,
        }
    }

    /// Given for `field`.
    pub fn on(field: impl Into<String>, message: impl Into<String>) -> Self {
        InputError {
            message: message.into(),
            field: Some(field.into()),
        }
    }
}

/// Something went wrong on the inside: logged in full, the user only told that it failed.
#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct InternalError {
    pub message: String,
}

impl InternalError {
    pub fn new(message: impl Into<String>) -> Self {
        InternalError {
            message: message.into(),
        }
    }
}

/// Which of the three an error is, as whoever answers a caller sorts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    Business,
    Input { field: Option<String> },
    Internal,
}

/// Sort an error: one marked as input or internal is; one the database or the ORM's own
/// limits raised is internal; any other — the plain messages plugins refuse with — is business.
pub fn kind_of(error: &(dyn std::error::Error + 'static)) -> ErrorKind {
    if let Some(input) = error.downcast_ref::<InputError>() {
        return ErrorKind::Input {
            field: input.field.clone(),
        };
    }
    let internal = error.is::<InternalError>()
        || error.is::<crate::database::ErrorType>()
        || error.is::<postgres::Error>()
        || error.is::<MaximumRecursionDepthCompute>()
        || error.is::<MaximumCallDepth>();
    if internal {
        ErrorKind::Internal
    } else {
        ErrorKind::Business
    }
}
