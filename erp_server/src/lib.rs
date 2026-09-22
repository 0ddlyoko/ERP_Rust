//! The HTTP side of the JSON-RPC interface.
//!
//! Only the transport: turning a request into a response belongs to `erp::jsonrpc`, which knows
//! nothing about HTTP and is tested without a socket. This carries bytes, decides how many
//! requests run at once, and nothing else.
//!
//! Kept out of `erp` so the core stays free of an async runtime: an embedder, a test or a batch
//! job uses the ORM without ever pulling in a server.

use axum::Router;
use axum::extract::{ConnectInfo, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use erp::app::Application;
use erp::jsonrpc;
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
/// One for now. Work that never reaches the database — a file to move, an answer from memory —
/// belongs on a route of its own, which is also how it escapes the queue this one waits in.
pub fn router(server: Arc<Server>) -> Router {
    Router::new()
        .route("/jsonrpc", post(call))
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
    body: String,
) -> Response {
    let started = Instant::now();
    let asked = asked_for(&body);

    // Taken before any thread is occupied: a request waiting its turn is a suspended future, not
    // a parked worker.
    let Ok(_permit) = Arc::clone(&server.permits).acquire_owned().await else {
        return (StatusCode::SERVICE_UNAVAILABLE, "shutting down").into_response();
    };

    let app = Arc::clone(&server.app);
    // The ORM is synchronous down to the database driver, so the work happens on a thread that
    // is allowed to block rather than on the runtime's.
    //
    // Nobody is identified yet: every call runs as the system until sessions land.
    let answer = tokio::task::spawn_blocking(move || jsonrpc::handle(&app, None, &body)).await;

    let response = match answer {
        // A notification is owed nothing, which over HTTP is an empty answer rather than an
        // empty body with a content type promising JSON.
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
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
        "{caller} POST /jsonrpc {} {asked} {:.1?}",
        response.status().as_u16(),
        started.elapsed(),
    );
    response
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
