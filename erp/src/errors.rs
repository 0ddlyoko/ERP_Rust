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

/// Why something failed, sorted the way whoever answers a caller needs it: a business rule
/// refusing, what the user gave not holding, or the server failing. Each holds the error itself
/// — a plugin's own type among them, found again with [`Error::downcast_ref`] — so nothing of it
/// is lost.
///
/// `?` sorts on its own: a message is a business refusal, JSON that does not parse is an input
/// one, an error of the database or of the ORM's own limits is internal, and a plugin says for
/// its own types with a `From`.
#[derive(Debug)]
pub enum Error {
    /// A business rule refuses what was asked: the user is told why, and may act on it.
    Business(Box<dyn std::error::Error + Send + Sync>),
    /// What the user gave does not hold, naming the field when there is one, for the client to
    /// point at it.
    Input {
        error: Box<dyn std::error::Error + Send + Sync>,
        field: Option<String>,
    },
    /// Something went wrong on the inside: logged in full, the user only told that it failed.
    Internal(Box<dyn std::error::Error + Send + Sync>),
}

/// A message, as the error a variant holds when there is nothing more to say.
#[derive(Debug, Clone, Error)]
#[error("{0}")]
pub struct Message(pub String);

impl Error {
    /// A business rule refusing, with this message.
    pub fn business(message: impl Into<String>) -> Self {
        Error::Business(Box::new(Message(message.into())))
    }

    /// What was given for `field` not holding.
    pub fn input(field: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Input {
            error: Box::new(Message(message.into())),
            field: Some(field.into()),
        }
    }

    /// The server failing, with this message for the log.
    pub fn internal(message: impl Into<String>) -> Self {
        Error::Internal(Box::new(Message(message.into())))
    }

    /// The error a variant holds.
    pub fn inner(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        match self {
            Error::Business(error) | Error::Internal(error) | Error::Input { error, .. } => {
                &**error
            }
        }
    }

    /// The error held, as the type it was raised with: a plugin's own, an [`AccessDenied`].
    ///
    /// [`AccessDenied`]: crate::access::AccessDenied
    pub fn downcast_ref<T: std::error::Error + 'static>(&self) -> Option<&T> {
        self.inner().downcast_ref::<T>()
    }

    /// Whether the error held is of that type.
    pub fn is<T: std::error::Error + 'static>(&self) -> bool {
        self.downcast_ref::<T>().is_some()
    }

    /// Which of the three it is.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Business(_) => ErrorKind::Business,
            Error::Input { field, .. } => ErrorKind::Input {
                field: field.clone(),
            },
            Error::Internal(_) => ErrorKind::Internal,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.inner(), f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.inner())
    }
}

/// Which of the three an [`Error`] is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    Business,
    Input { field: Option<String> },
    Internal,
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Error::business(message)
    }
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Error::business(message)
    }
}

/// An error boxed by a crate below the ORM: an [`Error`] boxed on the way comes back as it was,
/// JSON that does not parse is an input one, an error of the database or of the ORM's own limits
/// is internal, any other a business one — the plain messages those crates refuse with.
impl From<Box<dyn std::error::Error + Send + Sync>> for Error {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        let error = match error.downcast::<Error>() {
            Ok(error) => return *error,
            Err(error) => error,
        };
        if error.is::<serde_json::Error>() {
            return Error::Input { error, field: None };
        }
        let internal = error.is::<crate::database::ErrorType>()
            || error.is::<postgres::Error>()
            || error.is::<std::io::Error>()
            || error.is::<erp_types::field::HashError>()
            || error.is::<MaximumRecursionDepthCompute>()
            || error.is::<MaximumCallDepth>();
        if internal {
            Error::Internal(error)
        } else {
            Error::Business(error)
        }
    }
}

macro_rules! sorted {
    ($variant:ident: $($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for Error {
                fn from(error: $ty) -> Self {
                    Error::$variant(Box::new(error))
                }
            }
        )*
    };
}

sorted!(Internal:
    crate::database::ErrorType,
    postgres::Error,
    std::io::Error,
    std::num::ParseIntError,
    std::num::TryFromIntError,
    MaximumRecursionDepthCompute,
    MaximumCallDepth,
    erp_types::field::HashError,
);
sorted!(Business:
    MissingRecords,
    crate::access::AccessDenied,
    crate::http::HttpError,
    crate::model::ModelNotFound,
    crate::model::MethodNotRegistered,
    crate::model::FieldNotFound,
    crate::model::MethodNotExposed,
    crate::http::ParamError,
    crate::data::DataError,
    crate::xml::XmlError,
    crate::util::dependency::CircularDependencyError,
    erp_types::field::ParseFieldTypeError,
);

/// JSON that does not parse is what the caller sent: a request's body, a call's parameters.
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Error::Input {
            error: Box::new(error),
            field: None,
        }
    }
}
