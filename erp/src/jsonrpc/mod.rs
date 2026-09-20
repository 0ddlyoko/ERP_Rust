//! The JSON-RPC 2.0 interface.
//!
//! Transport-free on purpose: this turns a request into a response, and knows nothing about HTTP.
//! What carries the bytes is a separate concern, and testing the protocol should not need a
//! socket.
//!
//! A method name is `model.operation`. The operation is one of the protocol's own — `search`,
//! `read`, `read_matching`, `count`, `create`, `write`, `delete` — or a method the model exposed
//! with `#[erp(rpc)]`. Nothing else is reachable.

mod message;
mod verbs;

pub use message::*;
pub use verbs::{Verb, is_reserved, reserved_names};

use crate::app::Application;
use serde_json::Value;

/// Answer one request, or a batch of them.
///
/// Every call waits its turn at the application's gate, because every one of them reaches the
/// database. Work that does not — moving a file, answering from memory — does not come through
/// here and is not held back by a queue it has no part in.
///
/// `None` when nothing is owed: a lone notification, or a batch of them.
pub fn handle(app: &Application, uid: Option<u32>, body: &str) -> Option<Value> {
    let incoming: Incoming = match serde_json::from_str(body) {
        Ok(incoming) => incoming,
        Err(error) => {
            return Some(
                serde_json::to_value(Response::failed(
                    RpcError::parse_error(error.to_string()),
                    None,
                ))
                .expect("a response is always serialisable"),
            );
        }
    };

    match incoming {
        Incoming::Single(request) => answer(app, uid, request)
            .map(|response| serde_json::to_value(response).expect("serialisable")),
        Incoming::Batch(requests) if requests.is_empty() => Some(
            serde_json::to_value(Response::failed(
                RpcError::invalid_request("a batch must hold at least one request"),
                None,
            ))
            .expect("serialisable"),
        ),
        // Each request of a batch gets its own environment, and so its own transaction: the
        // specification treats them as independent calls that merely travelled together, and one
        // failing must not undo the rest. A batch that has to be all or nothing is a different
        // operation, not this one.
        Incoming::Batch(requests) => {
            let answers: Vec<Value> = requests
                .into_iter()
                .filter_map(|request| answer(app, uid, request))
                .map(|response| serde_json::to_value(response).expect("serialisable"))
                .collect();
            (!answers.is_empty()).then_some(Value::Array(answers))
        }
    }
}

/// Answer one request. `None` for a notification, which is owed nothing.
fn answer(app: &Application, uid: Option<u32>, request: Request) -> Option<Response> {
    let id = request.id.clone();
    let is_notification = id.is_none();

    let outcome = run(app, uid, &request);
    if is_notification {
        return None;
    }
    Some(match outcome {
        Ok(result) => Response::ok(result, id),
        Err(error) => Response::failed(error, id),
    })
}

fn run(app: &Application, uid: Option<u32>, request: &Request) -> Result<Value, RpcError> {
    if request.jsonrpc != "2.0" {
        return Err(RpcError::invalid_request(
            "only JSON-RPC 2.0 is spoken here",
        ));
    }
    let Some((model_name, operation)) = request.method.rsplit_once('.') else {
        return Err(RpcError::method_not_found(&request.method));
    };
    if app.model_manager.try_get_model(model_name).is_err()
        || app
            .model_manager
            .rpc
            .resolve(model_name, operation)
            .is_err()
    {
        return Err(RpcError::method_not_found(&request.method));
    }

    // Taken before the connection, so a request queuing for its turn is not also holding one of
    // the pool's connections open while it waits.
    let _pass = app.gate.enter();

    // One environment per request, so one transaction per request: it commits when the call
    // returns, and rolls back by being dropped when it does not.
    let mut env = app
        .new_env_as_option(uid)
        .map_err(|error| RpcError::internal(error.to_string()))?;

    let outcome = env.call_rpc(model_name, operation, &request.params);

    match outcome {
        Ok(result) => env
            .close()
            .map(|()| result)
            .map_err(|error| RpcError::internal(error.to_string())),
        // Dropping the environment rolls the transaction back, so nothing a failed call did
        // survives it.
        //
        // A call that failed reading its own parameters is the caller's mistake, not the
        // operation's, and the specification has a code for each.
        Err(error) => Err(if error.downcast_ref::<serde_json::Error>().is_some() {
            RpcError::invalid_params(error.to_string())
        } else {
            RpcError::call_failed(error.to_string())
        }),
    }
}
