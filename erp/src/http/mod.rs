//! Controllers: methods of plugins that answer URLs.
//!
//! Transport-free, like [`crate::jsonrpc`]: this turns a [`Request`] into a [`Response`] and knows
//! nothing about sockets. A controller is overridden the way a model is — another plugin declares
//! the same controller id and a method of the same name, and reaches the one below through
//! `sup` — so what a URL does can be changed without touching the plugin that declared it.

mod errors;
mod params;
mod request;
mod response;
mod routes;

pub use errors::HttpError;
pub use params::{FromParam, ParamError, find_record};
pub use request::Request;
pub use response::Response;
pub use routes::{Controller, ControllerRegistry, HasRoutes, HttpFn, Resolution};

use crate::access::AccessDenied;
use crate::app::Application;

/// Answer one request.
///
/// One environment, so one transaction, per request: committed when the controller answers, and
/// rolled back when it fails, so nothing a failed request wrote survives it. The caller is the
/// user nobody authenticated as until authentication reaches controllers.
pub fn handle(app: &Application, request: Request) -> Response {
    let (call, params) = match app
        .model_manager
        .controllers
        .resolve(request.method(), request.path())
    {
        Resolution::Found { call, params } => (call, params),
        Resolution::NotFound => return refusal(&HttpError::not_found("Nothing is served here")),
        Resolution::MethodNotAllowed(allowed) => {
            return refusal(&HttpError::new(405, "This URL does not answer that method"))
                .with_header("Allow", &allowed.join(", "));
        }
    };
    let request = request.with_path_params(params);

    let mut env = match app.new_env() {
        Ok(env) => env,
        Err(error) => return failure(&*error),
    };
    match call(&mut env, &request) {
        Ok(response) => match env.close() {
            Ok(()) => response,
            Err(error) => failure(&*error),
        },
        // Dropping the environment rolls the transaction back.
        Err(error) => {
            if let Some(refused) = error.downcast_ref::<HttpError>() {
                refusal(refused)
            } else if let Some(denied) = error.downcast_ref::<AccessDenied>() {
                refusal(&HttpError::new(403, denied.to_string()))
            } else {
                failure(&*error)
            }
        }
    }
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
