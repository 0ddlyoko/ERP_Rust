use crate::http::params::ParamError;
use thiserror::Error;

/// A refusal a controller answers with a status of its own.
///
/// Returned as an error, so a controller can stop with `?` wherever it finds the problem; the
/// message is shown to the caller, which is why it must not carry anything internal.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct HttpError {
    pub status: u16,
    pub message: String,
}

impl HttpError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        HttpError {
            status,
            message: message.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        HttpError::new(404, message)
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        HttpError::new(400, message)
    }

    /// A parameter that could not be read, named so the caller knows which one to fix.
    ///
    /// Answered as not found when it names no record: the URL points at nothing.
    pub fn bad_parameter(name: &str, error: &ParamError) -> Self {
        let status = match error {
            ParamError::NoSuchRecord => 404,
            _ => 400,
        };
        HttpError::new(status, format!("Parameter {name} cannot be read: {error}"))
    }

    /// Say which parameter a reading error came from, when it is the caller's mistake.
    pub fn about_parameter(
        name: &str,
        error: Box<dyn std::error::Error + Send + Sync>,
    ) -> Box<dyn std::error::Error + Send + Sync> {
        match error.downcast_ref::<ParamError>() {
            Some(param) => HttpError::bad_parameter(name, param).into(),
            None => error,
        }
    }
}
