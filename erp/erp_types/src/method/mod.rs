//! Overridable methods.
//!
//! A method declared overridable is not called directly. Several structs may contribute an
//! implementation of the same method to the same model, and what a call reaches is the most
//! derived one — the same relationship computed fields already have, generalised.

mod chain;
mod tag;

pub use chain::*;
pub use tag::*;
