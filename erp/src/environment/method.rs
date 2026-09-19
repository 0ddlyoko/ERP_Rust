//! Calling an overridable method.

use super::{Environment, Result};
use crate::model::MethodNotRegistered;
use erp_types::field::MultipleIds;
use erp_types::method::{MethodFn, Super};

impl<'mm> Environment<'mm> {
    /// Implementations of a method, most-derived first.
    ///
    /// Borrowed from the registry rather than from this environment, so a caller can hold the
    /// chain and still hand the environment to it.
    pub fn method_chain<A, R>(
        &self,
        model_name: &str,
        method_name: &str,
    ) -> Result<&'mm [MethodFn<A, R>]>
    where
        A: 'static,
        R: 'static,
    {
        let missing = || MethodNotRegistered {
            model_name: model_name.to_string(),
            method_name: method_name.to_string(),
        };
        let model = self
            .model_manager
            .try_get_model(model_name)
            .map_err(|_| missing())?;
        model
            .methods
            .chain::<A, R>(method_name)
            .ok_or_else(|| missing().into())
    }

    /// Call a method from the top of its chain.
    ///
    /// This is what an overridable method's own name resolves to, which is why calling one method
    /// from inside another reaches the most derived implementation rather than the neighbouring
    /// one.
    pub fn call_method<A, R>(
        &mut self,
        model_name: &str,
        method_name: &str,
        ids: &MultipleIds,
        args: &A,
    ) -> Result<R>
    where
        A: 'static,
        R: 'static,
    {
        let chain = self.method_chain::<A, R>(model_name, method_name)?;
        Super::head(chain, ids, args)
            .try_call(self)?
            .ok_or_else(|| {
                MethodNotRegistered {
                    model_name: model_name.to_string(),
                    method_name: method_name.to_string(),
                }
                .into()
            })
    }
}
