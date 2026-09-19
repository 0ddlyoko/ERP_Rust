use crate::environment::ErasedEnvironment;
use crate::field::MultipleIds;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// One contribution to an overridable method.
///
/// Takes the recordset as ids rather than as `&Self`: contributors are different Rust types, so
/// they cannot share a pointer that names one of them. Each rebuilds its own type from the ids.
///
/// Arguments travel as a tuple rather than a named struct, so that the type is the same one in
/// every crate. That is what lets two independently compiled plugins land on the same chain
/// without either naming a type the other declared.
pub type MethodFn<A, R> =
    fn(MultipleIds, &mut dyn ErasedEnvironment, &A, Super<'_, A, R>) -> Result<R>;

/// Cursor over the implementations a method overrides.
///
/// Mirrors the cursor computed fields use. The distinction that matters: calling the method by
/// its own name restarts from the most derived implementation, whereas this walks one step down.
pub struct Super<'a, A, R> {
    remaining: &'a [MethodFn<A, R>],
    ids: &'a MultipleIds,
    args: &'a A,
}

// Written by hand rather than derived: `derive` would demand `A: Clone` and `R: Clone`, which a
// cursor holding only references has no use for.
impl<A, R> Clone for Super<'_, A, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, R> Copy for Super<'_, A, R> {}

impl<'a, A, R> Super<'a, A, R> {
    /// Cursor over a whole chain, most-derived first.
    pub fn head(chain: &'a [MethodFn<A, R>], ids: &'a MultipleIds, args: &'a A) -> Self {
        Self {
            remaining: chain,
            ids,
            args,
        }
    }

    /// Whether there is an implementation below this one.
    pub fn exists(&self) -> bool {
        !self.remaining.is_empty()
    }

    /// Records the chain is running on.
    pub fn ids(&self) -> &MultipleIds {
        self.ids
    }

    /// Arguments the chain was called with.
    pub fn args(&self) -> &A {
        self.args
    }

    /// Call the implementation this one overrides, on the same records and arguments.
    ///
    /// Yields the return type's default when there is none, so a method can call `super`
    /// unconditionally. Use [`Super::try_call`] when the default would be a lie.
    pub fn call(&self, env: &mut dyn ErasedEnvironment) -> Result<R>
    where
        R: Default,
    {
        Ok(self.try_call(env)?.unwrap_or_default())
    }

    /// Same, on a different set of records.
    pub fn call_on(&self, ids: impl Into<MultipleIds>, env: &mut dyn ErasedEnvironment) -> Result<R>
    where
        R: Default,
    {
        Ok(self.try_call_on(ids, env)?.unwrap_or_default())
    }

    /// Same, with different arguments.
    pub fn call_with(&self, args: &A, env: &mut dyn ErasedEnvironment) -> Result<R>
    where
        R: Default,
    {
        Ok(self.try_call_with(args, env)?.unwrap_or_default())
    }

    /// Call the implementation below, or report that there is none.
    ///
    /// `None` means the chain is exhausted, which a caller returning something that has no
    /// meaningful default needs to tell apart from a real result.
    pub fn try_call(&self, env: &mut dyn ErasedEnvironment) -> Result<Option<R>> {
        self.dispatch(self.ids.clone(), self.args, env)
    }

    /// Same, on a different set of records.
    ///
    /// Unlike a computed field, an empty recordset is not short-circuited: a method may
    /// legitimately have nothing to work on and still have something to say.
    pub fn try_call_on(
        &self,
        ids: impl Into<MultipleIds>,
        env: &mut dyn ErasedEnvironment,
    ) -> Result<Option<R>> {
        self.dispatch(ids.into(), self.args, env)
    }

    /// Same, with different arguments.
    pub fn try_call_with(&self, args: &A, env: &mut dyn ErasedEnvironment) -> Result<Option<R>> {
        self.dispatch(self.ids.clone(), args, env)
    }

    fn dispatch(
        &self,
        ids: MultipleIds,
        args: &A,
        env: &mut dyn ErasedEnvironment,
    ) -> Result<Option<R>> {
        let Some((next, remaining)) = self.remaining.split_first() else {
            return Ok(None);
        };
        next(
            ids.clone(),
            env,
            args,
            Super {
                remaining,
                ids: &ids,
                args,
            },
        )
        .map(Some)
    }
}
