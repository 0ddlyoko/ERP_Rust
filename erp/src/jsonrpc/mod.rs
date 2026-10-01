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
use crate::environment::Environment;
use serde_json::Value;

/// What identifies a call carried over HTTP: the bearer token, or else the browser's session
/// cookie.
///
/// The cookie only with the page's CSRF token in `X-CSRF-Token`: a browser sends its cookies
/// with a request any site makes it send, and a header of this site's only with this site's
/// script. A bearer token needs none, since another site cannot make the browser present one.
/// Neither is nobody in particular, which needs no token either.
pub fn credentials(
    app: &Application,
    bearer: Option<&str>,
    request: &crate::http::Request,
) -> Result<Option<String>, RpcError> {
    if let Some(token) = bearer {
        return Ok(Some(token.to_string()));
    }
    let Some(session) = request
        .cookie(crate::http::SESSION_COOKIE)
        .filter(|session| !session.is_empty())
    else {
        return Ok(None);
    };
    if !crate::http::csrf::is_from_the_site(app, request) {
        return Err(RpcError::csrf_refused());
    }
    Ok(Some(session.to_string()))
}

/// Answer one request, or a batch of them.
///
/// How many of these run at once is decided by whoever schedules them, not here: waiting for a
/// turn should not cost a thread, and only the scheduler knows how to wait cheaply.
///
/// `credentials` is the bearer token the caller presented, if any. Resolving it needs the
/// database, so it is passed down rather than resolved by whoever carried the bytes: doing it up
/// there would mean a second environment, and so a second connection and transaction, for every
/// request.
///
/// `None` when nothing is owed: a lone notification, or a batch of them.
pub fn handle(app: &Application, credentials: Option<&str>, body: &str) -> Option<Value> {
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
        Incoming::Single(request) => answer(app, credentials, request)
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
                .filter_map(|request| answer(app, credentials, request))
                .map(|response| serde_json::to_value(response).expect("serialisable"))
                .collect();
            (!answers.is_empty()).then_some(Value::Array(answers))
        }
    }
}

/// Answer one request. `None` for a notification, which is owed nothing.
fn answer(app: &Application, credentials: Option<&str>, request: Request) -> Option<Response> {
    let id = request.id.clone();
    let is_notification = id.is_none();

    let outcome = run(app, credentials, &request);
    if is_notification {
        return None;
    }
    Some(match outcome {
        Ok(result) => Response::ok(result, id),
        Err(error) => Response::failed(error, id),
    })
}

/// Who a token identifies, or a refusal.
///
/// A failure to answer is not the same as an answer of nobody: the caller is told the request
/// could not be handled, and what actually broke goes to the log, where a database's own words
/// about a row cannot reach whoever was guessing.
fn identify(
    app: &Application,
    env: &mut Environment,
    token: &str,
) -> Result<Option<u32>, RpcError> {
    let Some(resolve) = app.model_manager.identities.resolver() else {
        return Err(RpcError::unauthorized());
    };
    // As root, not as whoever the caller turns out to be: looking a session up is the process
    // asking on its own account, before anybody is identified, and it must not depend on what the
    // user nobody authenticated as happens to be allowed to read.
    let mut root = env
        .as_root()
        .map_err(|error| RpcError::internal(error.to_string()))?;
    match resolve(&mut root, token) {
        Ok(Some(uid)) => Ok(Some(uid)),
        Ok(None) => Err(RpcError::unauthorized()),
        Err(error) => {
            tracing::error!(%error, "Cannot resolve a caller's token");
            Err(RpcError::internal("Cannot resolve the caller's identity"))
        }
    }
}

fn run(app: &Application, credentials: Option<&str>, request: &Request) -> Result<Value, RpcError> {
    if request.jsonrpc != "2.0" {
        return Err(RpcError::invalid_request(
            "only JSON-RPC 2.0 is spoken here",
        ));
    }
    let Some((model_name, operation)) = request.method.rsplit_once('.') else {
        return Err(RpcError::method_not_found(&request.method));
    };

    // One environment per request, so one transaction per request: it commits when the call
    // returns, and rolls back by being dropped when it does not.
    let mut env = app
        .new_env()
        .map_err(|error| RpcError::internal(error.to_string()))?;

    // Before the method is even looked up. Who is asking is settled first, so that what exists
    // is not something an unidentified caller can map out by trying names.
    let caller = match credentials {
        Some(token) => identify(app, &mut env, token)?,
        None => env.uid(),
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

    let outcome = match caller {
        Some(uid) => env
            .as_user(uid)
            .call_rpc(model_name, operation, &request.params),
        None => env.call_rpc(model_name, operation, &request.params),
    };

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
