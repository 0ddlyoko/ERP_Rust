//! Overridable methods.
//!
//! Several structs may contribute an implementation of the same method to the same model, and
//! what a call reaches is the most derived one — the relationship computed fields already have,
//! generalised.
//!
//! Contributors find each other by the model and the method name, exactly as they do for a field.
//! Nothing has to name anything the other declared, which is what lets two plugins that know
//! nothing of one another extend the same method.

mod chain;

pub use chain::*;

use std::fmt;

/// What a method works on, which every contributor to it has to agree on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Receiver {
    /// Records: `&self` on `Model<MultipleIds>`, as many as it is called on.
    Records,
    /// One record: `&self` on `Model<SingleId>`, called on exactly one.
    Record,
    /// The model itself, without records: a method taking no `self`, as Odoo's `@api.model`.
    Model,
}

impl fmt::Display for Receiver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Receiver::Records => "records (Model<MultipleIds>)",
            Receiver::Record => "one record (Model<SingleId>)",
            Receiver::Model => "the model, without self",
        })
    }
}
