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
