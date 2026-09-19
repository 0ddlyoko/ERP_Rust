//! Methods a remote caller may reach.
//!
//! Kept apart from the override registry on purpose. Being overridable means another plugin can
//! extend a method; being callable over RPC means whatever is on the other end of a socket can
//! run it. A method is in this registry only because it was marked, so one that was never marked
//! does not exist over the wire — it is not forbidden, there is nothing to forbid.

use crate::environment::Environment;
use erp_types::field::MultipleIds;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use thiserror::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// A marked method, wrapped so its arguments arrive as JSON and its result leaves as JSON.
///
/// The wrapper calls the method by its own name, so a remote call goes through the override
/// chain exactly like an internal one.
pub type RpcFn = fn(MultipleIds, &Value, &mut Environment) -> Result<Value>;

#[derive(Debug, Clone, Error)]
#[error("Method \"{model_name}\".\"{method_name}\" cannot be called remotely")]
pub struct MethodNotExposed {
    pub model_name: String,
    pub method_name: String,
}

/// Every method reachable from outside the process.
#[derive(Default)]
pub struct RpcRegistry {
    methods: HashMap<(String, String), RpcFn>,
}

impl RpcRegistry {
    pub fn register(&mut self, model_name: &str, method_name: &str, call: RpcFn) {
        self.methods
            .insert((model_name.to_string(), method_name.to_string()), call);
    }

    /// The wrapper for a method, if it was marked.
    ///
    /// The same answer for a method that does not exist and one that exists but was not marked:
    /// what a caller may reach should not tell them what else is there.
    pub fn get(&self, model_name: &str, method_name: &str) -> Result<RpcFn> {
        self.methods
            .get(&(model_name.to_string(), method_name.to_string()))
            .copied()
            .ok_or_else(|| {
                MethodNotExposed {
                    model_name: model_name.to_string(),
                    method_name: method_name.to_string(),
                }
                .into()
            })
    }

    /// Every exposed method, as `model.method`, for whoever needs to list them.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .methods
            .keys()
            .map(|(model, method)| format!("{model}.{method}"))
            .collect();
        names.sort();
        names
    }
}
