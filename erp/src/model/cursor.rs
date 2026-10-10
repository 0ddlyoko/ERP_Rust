use crate::Result;
use erp_types::environment::ErasedEnvironment;
use erp_types::field::MultipleIds;
use erp_types::method::IntoArgs;

/// Cursor over the implementations a method overrides, as an override is given it: the chain's
/// own cursor, answering with an [`Error`](crate::Error), so `sup.call(env)` ends a method as it is.
pub struct Super<'a, A, R>(erp_types::method::Super<'a, A, R>);

impl<A, R> Clone for Super<'_, A, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, R> Copy for Super<'_, A, R> {}

impl<'a, A, R> Super<'a, A, R> {
    /// The chain's cursor, as an override is given it.
    pub fn new(cursor: erp_types::method::Super<'a, A, R>) -> Self {
        Super(cursor)
    }

    /// Whether there is an implementation below this one.
    pub fn exists(&self) -> bool {
        self.0.exists()
    }

    /// Records the chain is running on.
    pub fn ids(&self) -> &MultipleIds {
        self.0.ids()
    }

    /// Arguments the chain was called with.
    pub fn args(&self) -> &A {
        self.0.args()
    }

    /// Call the implementation this one overrides, on the same records and arguments; the
    /// return type's default when there is none.
    pub fn call(&self, env: &mut dyn ErasedEnvironment) -> Result<R>
    where
        R: Default,
    {
        Ok(self.0.call(env)?)
    }

    /// Same, on a different set of records.
    pub fn call_on(&self, ids: impl Into<MultipleIds>, env: &mut dyn ErasedEnvironment) -> Result<R>
    where
        R: Default,
    {
        Ok(self.0.call_on(ids, env)?)
    }

    /// Same, with different arguments: the value alone for a method taking one, a tuple of them
    /// otherwise.
    pub fn call_with(&self, args: impl IntoArgs<A>, env: &mut dyn ErasedEnvironment) -> Result<R>
    where
        R: Default,
    {
        Ok(self.0.call_with(args, env)?)
    }

    /// Call the implementation below, or report that there is none.
    pub fn try_call(&self, env: &mut dyn ErasedEnvironment) -> Result<Option<R>> {
        Ok(self.0.try_call(env)?)
    }

    /// Same, on a different set of records.
    pub fn try_call_on(
        &self,
        ids: impl Into<MultipleIds>,
        env: &mut dyn ErasedEnvironment,
    ) -> Result<Option<R>> {
        Ok(self.0.try_call_on(ids, env)?)
    }

    /// Same, with different arguments.
    pub fn try_call_with(
        &self,
        args: impl IntoArgs<A>,
        env: &mut dyn ErasedEnvironment,
    ) -> Result<Option<R>> {
        Ok(self.0.try_call_with(args, env)?)
    }
}
