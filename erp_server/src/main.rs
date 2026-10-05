//! The server.

use erp::app::{Application, LaunchArgs};
use erp::config::Config;
use std::error::Error;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

/// How much stack each thread handling a request gets: well above what any request needs, so a
/// deep but legitimate one finishes. Input that could recurse without end is bounded before it
/// is followed, since running out of stack takes the whole process down.
const REQUEST_STACK_SIZE: usize = 16 * 1024 * 1024;

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(REQUEST_STACK_SIZE)
        .build()?
        .block_on(serve())
}

async fn serve() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Verbosity is driven by RUST_LOG, defaulting to `info` when it is unset.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    log_panics();

    let config = Config::try_default()?;
    // The file says where to listen; the variable is there to override it for one run without
    // editing anything.
    let address: SocketAddr = match std::env::var("ERP_LISTEN") {
        Ok(listen) => listen.parse()?,
        Err(_) => config.server.address()?,
    };

    let mut app = Application::new(config);
    let launch = LaunchArgs::from_args(std::env::args().skip(1))?;
    app.set_install(launch.install);
    app.set_data_update(launch.update);
    erp_server::load_and_serve(app, address).await
}

/// Write every panic to the log: what it said, where, and the calls that led there. The request
/// it happened in fails alone; whoever runs the server still needs to see why.
fn log_panics() {
    std::panic::set_hook(Box::new(|info| {
        let message = info.payload_as_str().unwrap_or("a panic without a message");
        let location = info
            .location()
            .map(|location| location.to_string())
            .unwrap_or_default();
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!("Panicked at {location}: {message}\n{backtrace}");
    }));
}
