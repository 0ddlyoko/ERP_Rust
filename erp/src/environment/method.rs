//! Calling an overridable method.

use super::{Environment, MAX_CALL_DEPTH, Result};
use crate::errors::MaximumCallDepth;
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
        self.enter_call(model_name, method_name)?;
        let result = Super::head(chain, ids, args).try_call(self);
        self.call_stack.pop();

        result?.ok_or_else(|| {
            MethodNotRegistered {
                model_name: model_name.to_string(),
                method_name: method_name.to_string(),
            }
            .into()
        })
    }

    /// Record that a method is running, and refuse to go deeper than the stack can take.
    ///
    /// Checked before pushing, so the reported path is the one that led here rather than one
    /// frame past it.
    fn enter_call(&mut self, model_name: &str, method_name: &str) -> Result<()> {
        if self.call_stack.len() >= MAX_CALL_DEPTH {
            // The tail rather than the whole stack: a cycle repeats, and two hundred lines of it
            // say nothing the last dozen do not.
            let path = self
                .call_stack
                .iter()
                .skip(self.call_stack.len().saturating_sub(12))
                .map(|(model, method)| format!("{model}.{method}"))
                .chain(std::iter::once(format!("{model_name}.{method_name}")))
                .collect();
            return Err(MaximumCallDepth {
                limit: MAX_CALL_DEPTH,
                path,
            }
            .into());
        }
        self.call_stack
            .push((model_name.to_string(), method_name.to_string()));
        Ok(())
    }

    /// Call a method a remote caller named.
    ///
    /// Goes through the wrapper the method was exposed with, which calls it by its own name — so
    /// a remote call reaches the most derived implementation, exactly like an internal one.
    pub fn call_rpc(
        &mut self,
        model_name: &str,
        method_name: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let call = self.model_manager.rpc.resolve(model_name, method_name)?;
        call(self, model_name, params)
    }
}
