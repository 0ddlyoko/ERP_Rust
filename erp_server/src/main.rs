//! The server.

use erp::app::{Application, DataUpdate};
use erp::config::Config;
use std::error::Error;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Verbosity is driven by RUST_LOG, defaulting to `info` when it is unset.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::try_default()?;
    // The file says where to listen; the variable is there to override it for one run without
    // editing anything.
    let address: SocketAddr = match std::env::var("ERP_LISTEN") {
        Ok(listen) => listen.parse()?,
        Err(_) => config.server.address()?,
    };

    let mut app = Application::new(config);
    app.set_data_update(DataUpdate::from_args(std::env::args().skip(1))?);
    erp_server::load_and_serve(app, address).await
}
