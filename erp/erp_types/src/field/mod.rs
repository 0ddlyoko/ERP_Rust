mod error;
mod field_compute;
mod field_depends;
mod field_descriptor;
mod field_reference;
mod field_type;
mod id;
mod reference;

pub use error::*;
pub use field_compute::*;
pub use field_depends::*;
pub use field_descriptor::*;
pub use field_reference::*;
pub use field_type::*;
pub use id::*;
pub use reference::*;

/// Re-exported so generated model code and plugins can name the field types without depending on
/// `chrono` and `rust_decimal` directly.
pub use chrono::{DateTime, NaiveDate, Utc};
pub use rust_decimal::Decimal;

/// Timestamp field type.
///
/// Aliased because the derive macro keys a field on the last path segment of its type and drops
/// generic arguments, so `DateTime<Utc>` cannot be written directly in a model.
pub type Timestamp = DateTime<Utc>;
