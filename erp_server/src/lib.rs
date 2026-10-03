//! The HTTP side of the JSON-RPC interface and of the controllers.
//!
//! Only the transport: turning a request into a response belongs to `erp::jsonrpc` and
//! `erp::http`, which know nothing about sockets and are tested without one. This carries bytes,
//! decides how many requests run at once, and nothing else.
//!
//! Kept out of `erp` so the core stays free of an async runtime: an embedder, a test or a batch
//! job uses the ORM without ever pulling in a server.

use axum::Router;
use axum::body::Bytes;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use erp::app::Application;
use erp::request_log::{self, RequestLog};
use erp::{http, jsonrpc};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;

/// Everything a request needs.
pub struct Server {
    app: Arc<Application>,
    /// How many requests may be served at once.
    ///
    /// Asynchronous on purpose. The limit exists because serving a request costs a database
    /// connection and a transaction, but *waiting* for a turn should cost nothing — a blocking
    /// primitive here would park a worker thread per queued request, so a hundred requests
    /// behind a limit of five would hold ninety-five threads doing nothing.
    permits: Arc<Semaphore>,
}

impl Server {
    pub fn new(app: Application) -> Self {
        // Zero means no bound, which the semaphore expresses as more permits than requests can
        // ever be in flight.
        let permits = match app.max_concurrent_requests() {
            0 => Semaphore::MAX_PERMITS,
            limit => limit,
        };
        Self {
            app: Arc::new(app),
            permits: Arc::new(Semaphore::new(permits)),
        }
    }

    /// The application being served, for whoever needs to reach it directly.
    pub fn application(&self) -> &Application {
        &self.app
    }
}

/// The routes.
///
/// `/jsonrpc` for the protocol, and everything else for the controllers plugins declare. Work
/// that never reaches the database — a file to move, an answer from memory — belongs on a route
/// of its own, which is also how it escapes the queue these wait in.
pub fn router(server: Arc<Server>) -> Router {
    Router::new()
        .route("/jsonrpc", post(call))
        .fallback(controller)
        .with_state(server)
}

/// The routes, ready to serve, with each caller's address attached.
///
/// The one way to build the service, so that a test serves exactly what the binary does — the
/// address is an extractor, and a router built without it fails at the first request rather than
/// at compile time.
pub fn service(
    server: Arc<Server>,
) -> axum::extract::connect_info::IntoMakeServiceWithConnectInfo<Router, SocketAddr> {
    router(server).into_make_service_with_connect_info::<SocketAddr>()
}

/// Load an application from a thread that is allowed to block.
///
/// Loading opens a database connection, and the PostgreSQL driver is synchronous: it builds a
/// runtime of its own and blocks on it, which panics on a thread already driving one. Owning that
/// rule here rather than stating it in a comment is what keeps a caller from getting it wrong —
/// the same reason requests reach the driver through [`tokio::task::spawn_blocking`].
pub async fn load(mut app: Application) -> Result<Application, Box<dyn Error + Send + Sync>> {
    tokio::task::spawn_blocking(move || {
        app.load()?;
        Ok(app)
    })
    .await?
}

/// Load an application, then serve it until the process is asked to stop.
pub async fn load_and_serve(
    app: Application,
    address: SocketAddr,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let app = load(app).await?;
    tracing::info!(
        models = app.model_manager.get_models().len(),
        exposed = app.model_manager.rpc.names().len(),
        concurrency = app.max_concurrent_requests(),
        "Application loaded"
    );
    serve(app, address).await?;
    Ok(())
}

/// Serve an application that is already loaded, until the process is asked to stop.
pub async fn serve(app: Application, address: SocketAddr) -> std::io::Result<()> {
    let server = Arc::new(Server::new(app));
    let listener = tokio::net::TcpListener::bind(address).await?;
    // The address the socket actually got, not the one that was asked for: a port of 0 means the
    // operating system picks one, and that is the number somebody needs to read.
    let bound = listener.local_addr()?;
    tracing::info!("Running on http://{bound}");

    axum::serve(listener, service(server))
        .with_graceful_shutdown(interrupted())
        .await
}

async fn interrupted() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("Shutting down");
}

async fn call(
    State(server): State<Arc<Server>>,
    ConnectInfo(caller): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let started = Instant::now();
    let asked = asked_for(&body);
    let mut carried = http::Request::new("POST", "/jsonrpc");
    for (name, value) in &headers {
        if let Ok(value) = value.to_str() {
            carried = carried.with_header(name.as_str(), value);
        }
    }
    let token = match jsonrpc::credentials(&server.app, bearer(&headers).as_deref(), &carried) {
        Ok(token) => token,
        Err(refused) => {
            tracing::warn!("{caller} POST /jsonrpc refused: no valid CSRF token with the session");
            let answer = erp::serde_json::json!({"jsonrpc": "2.0", "error": refused, "id": null});
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json")],
                answer.to_string(),
            )
                .into_response();
        }
    };

    // Taken before any thread is occupied: a request waiting its turn is a suspended future, not
    // a parked worker.
    let Ok(_permit) = Arc::clone(&server.permits).acquire_owned().await else {
        return (StatusCode::SERVICE_UNAVAILABLE, "shutting down").into_response();
    };

    let app = Arc::clone(&server.app);
    // The ORM is synchronous down to the database driver, so the work happens on a thread that
    // is allowed to block rather than on the runtime's.
    //
    // The token travels no further than this: what it identifies is resolved down there, where a
    // database connection is already open.
    let answer = tokio::task::spawn_blocking(move || {
        request_log::start();
        let answer = jsonrpc::handle(&app, token.as_deref(), &body);
        (answer, request_log::current())
    })
    .await;
    let (answer, log) = match answer {
        Ok((answer, log)) => (Ok(answer), log),
        Err(error) => (Err(error), RequestLog::default()),
    };

    let response = match answer {
        // A notification is owed nothing, which over HTTP is an empty answer rather than an
        // empty body with a content type promising JSON.
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Ok(Some(value)) if names_nobody(&value) => (
            StatusCode::UNAUTHORIZED,
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::WWW_AUTHENTICATE, "Bearer"),
            ],
            value.to_string(),
        )
            .into_response(),
        Ok(Some(value)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            value.to_string(),
        )
            .into_response(),
        // The call panicked. What it was doing is already rolled back, because the environment
        // was dropped while unwinding.
        Err(error) => {
            tracing::error!(%error, "A call panicked");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/json")],
                r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error"},"id":null}"#
                    .to_string(),
            )
                .into_response()
        }
    };

    // One line per request, as a server is expected to keep. The method asked for is on it
    // because the path never varies: every call is a POST to the same place, and only the name
    // inside says what happened.
    tracing::info!(
        "{caller} POST /jsonrpc {} {asked} {} {:.1?}",
        response.status().as_u16(),
        described(&log),
        started.elapsed(),
    );
    response
}

/// A URL a controller answers, or a 404 saying none does.
///
/// Waits in the same queue as the protocol, since answering costs a database connection just the
/// same, and runs on a thread allowed to block for the same reason.
async fn controller(
    State(server): State<Arc<Server>>,
    ConnectInfo(caller): ConnectInfo<SocketAddr>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let started = Instant::now();
    let target = uri.path_and_query().map_or_else(
        || uri.path().to_string(),
        |target| target.as_str().to_string(),
    );
    let mut request = http::Request::new(method.as_str(), &target).with_body(body.to_vec());
    for (name, value) in &headers {
        if let Ok(value) = value.to_str() {
            request = request.with_header(name.as_str(), value);
        }
    }

    let Ok(_permit) = Arc::clone(&server.permits).acquire_owned().await else {
        return (StatusCode::SERVICE_UNAVAILABLE, "shutting down").into_response();
    };
    let app = Arc::clone(&server.app);
    let answer = tokio::task::spawn_blocking(move || {
        request_log::start();
        let answer = http::handle(&app, request);
        (answer, request_log::current())
    })
    .await;
    let (answer, log) = match answer {
        Ok((answer, log)) => (Ok(answer), log),
        Err(error) => (Err(error), RequestLog::default()),
    };

    let response = match answer {
        Ok(answer) => {
            let status =
                StatusCode::from_u16(answer.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let mut response = (status, answer.body().to_vec()).into_response();
            // The first of a name replaces what axum set by default; later ones, such as a second
            // `Set-Cookie`, are added beside it.
            let mut written = std::collections::HashSet::new();
            for (name, value) in answer.headers() {
                if let (Ok(name), Ok(value)) = (
                    header::HeaderName::try_from(name.as_str()),
                    header::HeaderValue::try_from(value.as_str()),
                ) {
                    if written.insert(name.clone()) {
                        response.headers_mut().insert(name, value);
                    } else {
                        response.headers_mut().append(name, value);
                    }
                }
            }
            response
        }
        Err(error) => {
            tracing::error!(%error, "A controller panicked");
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    };

    tracing::info!(
        "{caller} {method} {} {} {} {:.1?}",
        uri.path(),
        response.status().as_u16(),
        described(&log),
        started.elapsed(),
    );
    response
}

/// Who a request was answered for and the SQL it took: `uid=2 sql=12/3.4ms`, `uid=-` for nobody.
fn described(log: &RequestLog) -> String {
    let uid = log
        .uid
        .map_or_else(|| "-".to_string(), |uid| uid.to_string());
    format!(
        "uid={uid} sql={}/{:.1}ms",
        log.queries,
        log.sql_time.as_secs_f64() * 1000.0
    )
}

/// The bearer token a caller presented, if it presented one.
///
/// The scheme is matched without regard to case, which is what the specification for the header
/// asks for and what several clients rely on.
fn bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("Bearer")
        .then(|| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

/// Whether the answer is a refusal of the caller's identity.
///
/// The protocol layer knows nothing about statuses, so it says so with a code of its own and this
/// is where that code becomes the status HTTP already has for it. A batch is left alone: its
/// entries are independent calls, and one of them being refused is not the response's verdict.
fn names_nobody(answer: &erp::serde_json::Value) -> bool {
    answer
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(erp::serde_json::Value::as_i64)
        == Some(i64::from(erp::jsonrpc::RpcError::UNAUTHORIZED))
}

/// What a body asks for, for the log.
///
/// Read separately from the call itself, which costs one more pass over the body — microseconds
/// against a request that is about to talk to a database, and the difference between a log that
/// says something happened and one that says what.
fn asked_for(body: &str) -> String {
    let Ok(value) = erp::serde_json::from_str::<erp::serde_json::Value>(body) else {
        return "<unreadable>".to_string();
    };
    let name = |request: &erp::serde_json::Value| {
        request
            .get("method")
            .and_then(erp::serde_json::Value::as_str)
            .unwrap_or("<unnamed>")
            .to_string()
    };
    match &value {
        erp::serde_json::Value::Array(requests) => {
            let mut names: Vec<String> = requests.iter().map(name).collect();
            names.dedup();
            format!("batch({}) {}", requests.len(), names.join(","))
        }
        request => name(request),
    }
}
