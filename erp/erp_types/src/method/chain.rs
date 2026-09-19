use crate::environment::ErasedEnvironment;
use crate::field::MultipleIds;
use crate::method::MethodTag;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// One contribution to an overridable method.
///
/// Takes the recordset as ids rather than as `&Self`: contributors are different Rust types, so
/// they cannot share a pointer that names one of them. Each rebuilds its own type from the ids.
pub type MethodFn<T> = fn(
    MultipleIds,
    &mut dyn ErasedEnvironment,
    &<T as MethodTag>::Args,
    Super<'_, T>,
) -> Result<<T as MethodTag>::Ret>;

/// Cursor over the implementations a method overrides.
///
/// Mirrors the cursor computed fields use. The distinction that matters: calling the method by
/// its own name restarts from the most derived implementation, whereas this walks one step down.
pub struct Super<'a, T: MethodTag> {
    remaining: &'a [MethodFn<T>],
    ids: &'a MultipleIds,
    args: &'a T::Args,
}

// Written by hand rather than derived: `derive` would demand `T: Clone`, and a tag is a marker
// that never needs to be.
impl<T: MethodTag> Clone for Super<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: MethodTag> Copy for Super<'_, T> {}

impl<'a, T: MethodTag> Super<'a, T> {
    /// Cursor over a whole chain, most-derived first.
    pub fn head(chain: &'a [MethodFn<T>], ids: &'a MultipleIds, args: &'a T::Args) -> Self {
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
    pub fn args(&self) -> &T::Args {
        self.args
    }

    /// Call the implementation this one overrides, on the same records and arguments.
    ///
    /// Yields the return type's default when there is none, so a method can call `super`
    /// unconditionally. Use [`Super::try_call`] when the default would be a lie.
    pub fn call(&self, env: &mut dyn ErasedEnvironment) -> Result<T::Ret>
    where
        T::Ret: Default,
    {
        Ok(self.try_call(env)?.unwrap_or_default())
    }

    /// Same, on a different set of records.
    pub fn call_on(
        &self,
        ids: impl Into<MultipleIds>,
        env: &mut dyn ErasedEnvironment,
    ) -> Result<T::Ret>
    where
        T::Ret: Default,
    {
        Ok(self.try_call_on(ids, env)?.unwrap_or_default())
    }

    /// Same, with different arguments.
    pub fn call_with(&self, args: &T::Args, env: &mut dyn ErasedEnvironment) -> Result<T::Ret>
    where
        T::Ret: Default,
    {
        Ok(self.try_call_with(args, env)?.unwrap_or_default())
    }

    /// Call the implementation below, or report that there is none.
    ///
    /// `None` means the chain is exhausted, which a caller returning something that has no
    /// meaningful default needs to tell apart from a real result.
    pub fn try_call(&self, env: &mut dyn ErasedEnvironment) -> Result<Option<T::Ret>> {
        self.dispatch(self.ids.clone(), self.args, env)
    }

    /// Same, on a different set of records.
    ///
    /// Unlike a computed field, an empty recordset is not short-circuited: a method may legitimately
    /// have nothing to work on and still have something to say.
    pub fn try_call_on(
        &self,
        ids: impl Into<MultipleIds>,
        env: &mut dyn ErasedEnvironment,
    ) -> Result<Option<T::Ret>> {
        self.dispatch(ids.into(), self.args, env)
    }

    /// Same, with different arguments.
    pub fn try_call_with(
        &self,
        args: &T::Args,
        env: &mut dyn ErasedEnvironment,
    ) -> Result<Option<T::Ret>> {
        self.dispatch(self.ids.clone(), args, env)
    }

    fn dispatch(
        &self,
        ids: MultipleIds,
        args: &T::Args,
        env: &mut dyn ErasedEnvironment,
    ) -> Result<Option<T::Ret>> {
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
