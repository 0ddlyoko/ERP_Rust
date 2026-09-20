//! Methods a remote caller may reach.
//!
//! Kept apart from the override registry on purpose. Being overridable means another plugin can
//! extend a method; being callable over RPC means whatever is on the other end of a socket can
//! run it. A method is in this registry only because it was marked, so one that was never marked
//! does not exist over the wire — it is not forbidden, there is nothing to forbid.

use crate::environment::Environment;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use thiserror::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Anything a caller can reach by name: one of the protocol's own operations, or a method a
/// model exposed.
///
/// One signature for both, taking the request's parameters as they arrived, so that resolving a
/// name yields something callable without the caller having to know which kind it found. Each
/// one reads the shape it expects out of them.
///
/// A method's wrapper calls it by its own name, so a remote call goes through the override chain
/// exactly like an internal one.
pub type RpcFn = fn(&mut Environment, &str, &Value) -> Result<Value>;

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
    /// Expose a method.
    ///
    /// Panics when the name is one the protocol already answers to. The method would compile,
    /// register and never be reached, since the protocol's own operations are tried first — a
    /// startup that refuses to begin says so, a silent shadow does not.
    pub fn register(&mut self, model_name: &str, method_name: &str, call: RpcFn) {
        if crate::jsonrpc::is_reserved(method_name) {
            panic!(
                "Method \"{model_name}\".\"{method_name}\" cannot be exposed: {method_name} is \
                 one of the operations every model already answers to ({}). A call would never \
                 reach it.",
                crate::jsonrpc::reserved_names().join(", "),
            );
        }
        self.methods
            .insert((model_name.to_string(), method_name.to_string()), call);
    }

    /// What a name refers to on a model, whichever kind it turns out to be.
    ///
    /// The protocol's own operations come first, which is what makes their names reserved. The
    /// same answer for a method that does not exist and one that exists but was not exposed:
    /// what a caller may reach should not tell them what else is there.
    pub fn resolve(&self, model_name: &str, name: &str) -> Result<RpcFn> {
        if let Some(verb) = crate::jsonrpc::Verb::parse(name) {
            return Ok(verb.handler());
        }
        self.methods
            .get(&(model_name.to_string(), name.to_string()))
            .copied()
            .ok_or_else(|| {
                MethodNotExposed {
                    model_name: model_name.to_string(),
                    method_name: name.to_string(),
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
