use crate::environment::ErasedEnvironment;
use crate::field::MultipleIds;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// One contribution to a computed field.
///
/// Several structs may declare a compute for the same field; each becomes a link in a chain, and
/// receives a [`Super`] pointing at the ones it overrides.
pub type ComputeFn = fn(&str, MultipleIds, &mut dyn ErasedEnvironment, Super<'_>) -> Result<()>;

/// Cursor over the implementations a compute overrides.
///
/// Rust has no runtime inheritance, so the link to the previous implementation is passed
/// explicitly rather than looked up from a class hierarchy.
#[derive(Clone, Copy)]
pub struct Super<'a> {
    remaining: &'a [ComputeFn],
    field_name: &'a str,
    ids: &'a MultipleIds,
}

impl<'a> Super<'a> {
    /// Cursor over a whole chain, most-derived first.
    pub fn head(chain: &'a [ComputeFn], field_name: &'a str, ids: &'a MultipleIds) -> Self {
        Self {
            remaining: chain,
            field_name,
            ids,
        }
    }

    /// Call the implementation this one overrides.
    ///
    /// Does nothing when there is none, so a compute can call `super` unconditionally instead of
    /// testing for the base case. Not calling it at all replaces the previous implementation.
    pub fn call(&self, env: &mut dyn ErasedEnvironment) -> Result<()> {
        let Some((next, remaining)) = self.remaining.split_first() else {
            return Ok(());
        };
        next(
            self.field_name,
            self.ids.clone(),
            env,
            Super {
                remaining,
                field_name: self.field_name,
                ids: self.ids,
            },
        )
    }

    /// Whether there is an implementation below this one.
    pub fn exists(&self) -> bool {
        !self.remaining.is_empty()
    }

    /// Records the chain is running on.
    pub fn ids(&self) -> &MultipleIds {
        self.ids
    }
}
