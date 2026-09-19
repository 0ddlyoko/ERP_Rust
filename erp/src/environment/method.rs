//! Calling an overridable method.

use super::{Environment, Result};
use crate::model::MethodNotRegistered;
use erp_types::field::MultipleIds;
use erp_types::method::{MethodFn, MethodTag, Super, model_of};

impl<'mm> Environment<'mm> {
    /// Implementations of a method, most-derived first.
    ///
    /// Borrowed from the registry rather than from this environment, so a caller can hold the
    /// chain and still hand the environment to it.
    pub fn method_chain<T>(&self) -> Result<&'mm [MethodFn<T>]>
    where
        T: MethodTag,
    {
        let missing = || MethodNotRegistered {
            model_name: model_of::<T>().to_string(),
            method_name: T::NAME.to_string(),
        };
        let model = self
            .model_manager
            .try_get_model(model_of::<T>())
            .map_err(|_| missing())?;
        model.methods.chain::<T>().ok_or_else(|| missing().into())
    }

    /// Call a method from the top of its chain.
    ///
    /// This is what an overridable method's own name resolves to, which is why calling one method
    /// from inside another reaches the most derived implementation rather than the neighbouring
    /// one.
    pub fn call_method<T>(&mut self, ids: &MultipleIds, args: &T::Args) -> Result<T::Ret>
    where
        T: MethodTag,
    {
        let chain = self.method_chain::<T>()?;
        Super::head(chain, ids, args)
            .try_call(self)?
            .ok_or_else(|| {
                MethodNotRegistered {
                    model_name: model_of::<T>().to_string(),
                    method_name: T::NAME.to_string(),
                }
                .into()
            })
    }
}
