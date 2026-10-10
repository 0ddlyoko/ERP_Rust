//! Controllers: methods of plugins that answer URLs.
//!
//! Transport-free, like [`crate::jsonrpc`]: this turns a [`Request`] into a [`Response`] and knows
//! nothing about sockets. A controller is overridden the way a model is — another plugin declares
//! the same controller id and a method of the same name, and reaches the one below through
//! `sup` — so what a URL does can be changed without touching the plugin that declared it.

pub mod csrf;
mod errors;
mod params;
mod request;
mod response;
mod routes;

pub use errors::HttpError;
pub use params::{FromParam, ParamError, find_record};
pub use request::Request;
pub use response::Response;
pub use routes::{Auth, Controller, ControllerRegistry, HasRoutes, HttpFn, Resolution};

use crate::access::AccessDenied;
use crate::app::Application;
use crate::environment::Environment;

/// The cookie holding the token of the browser's session.
pub const SESSION_COOKIE: &str = "session_id";

/// Answer one request.
///
/// One environment, so one transaction, per request: committed when the controller answers, and
/// rolled back when it fails, so nothing a failed request wrote survives it. The caller is the
/// user the session cookie identifies; without one, or with one naming nobody — expired,
/// revoked — it is the user nobody authenticated as, and the controller decides what that may
/// see.
pub fn handle(app: &Application, request: Request) -> Response {
    let (call, params, needs_csrf, auth) = match app
        .model_manager
        .controllers
        .resolve(request.method(), request.path())
    {
        Resolution::Found {
            call,
            params,
            csrf,
            auth,
        } => (call, params, csrf, auth),
        Resolution::NotFound => return refusal(&HttpError::not_found("Nothing is served here")),
        Resolution::MethodNotAllowed(allowed) => {
            return refusal(&HttpError::new(405, "This URL does not answer that method"))
                .with_header("Allow", &allowed.join(", "));
        }
    };
    let binding = csrf::Binding::of(app.signing_secret(), &request);
    let request = request.with_path_params(params).with_csrf(binding.clone());
    if needs_csrf && !csrf::check(&binding, &request) {
        tracing::warn!(path = %request.path(), "A request without a valid CSRF token was refused");
        return refusal(&HttpError::bad_request(
            "Session expired (invalid CSRF token)",
        ));
    }
    let response = answer(app, call, auth, &request);
    match binding.cookie_to_set() {
        Some(cookie) => response.with_header("Set-Cookie", &cookie),
        None => response,
    }
}

/// Run the controller in its own transaction, as the caller its session cookie names — or as
/// nobody, without looking the session up, for a route answering everyone alike.
fn answer(app: &Application, call: HttpFn, auth: Auth, request: &Request) -> Response {
    let mut env = match app.new_env() {
        Ok(env) => env,
        Err(error) => return failure(&error),
    };
    let cookie = request
        .cookie(SESSION_COOKIE)
        .filter(|_| auth == Auth::User);
    let caller = match cookie {
        Some(token) => match identify(app, &mut env, token) {
            Ok(caller) => caller,
            Err(error) => return failure(&error),
        },
        None => None,
    };
    crate::request_log::identified(caller.or(env.uid()));
    // Saved while still the caller: what is left to work out on saving reads with their rights.
    let answer = match caller {
        Some(uid) => {
            let mut env = env.as_user(uid);
            call(&mut env, request).and_then(|response| env.save_all_to_db().map(|()| response))
        }
        None => {
            call(&mut env, request).and_then(|response| env.save_all_to_db().map(|()| response))
        }
    };
    match answer {
        Ok(response) => match env.close() {
            Ok(()) => response,
            Err(error) => failure(&error),
        },
        // Dropping the environment rolls the transaction back.
        Err(error) => {
            if let Some(refused) = error.downcast_ref::<HttpError>() {
                refusal(refused)
            } else if let Some(denied) = error.downcast_ref::<AccessDenied>() {
                refusal(&HttpError::new(403, denied.to_string()))
            } else if error.kind() == crate::errors::ErrorKind::Internal {
                failure(&error)
            } else {
                refusal(&HttpError::new(400, error.to_string()))
            }
        }
    }
}

/// Who a session token identifies, `None` for nobody.
///
/// As root: looking a session up is the process asking on its own account, before anybody is
/// identified.
fn identify(app: &Application, env: &mut Environment, token: &str) -> crate::Result<Option<u32>> {
    let Some(resolve) = app.model_manager.identities.resolver() else {
        return Ok(None);
    };
    resolve(&mut *env.as_root()?, token)
}

fn refusal(error: &HttpError) -> Response {
    Response::text(error.message.clone()).with_status(error.status)
}

/// A failure the caller cannot act on: logged in full, answered in general terms, so nothing
/// internal reaches whoever sent the request.
fn failure(error: &(dyn std::error::Error + Send + Sync)) -> Response {
    tracing::error!(%error, "A controller failed");
    Response::text("Internal Server Error").with_status(500)
}
