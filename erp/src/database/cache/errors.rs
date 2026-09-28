use thiserror::Error;

/// What the in-memory database refuses.
///
/// Typed, so that a test can tell one refusal from another instead of matching on a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CacheDatabaseError {
    #[error("Savepoint {name} is not the last one opened")]
    NotTheLastSavepoint { name: String },
    #[error("Cannot {operation} savepoint {name}: none is open")]
    MissingSavepoint {
        name: String,
        operation: &'static str,
    },
    #[error("No transaction to {operation}")]
    NoTransaction { operation: &'static str },
    #[error(
        "Operator {operator} does not apply to a relation as a whole; compare a field through it \
         instead"
    )]
    OperatorOnRelation { operator: String },
}
