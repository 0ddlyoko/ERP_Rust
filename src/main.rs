use erp::app::Application;
use erp::config::Config;
use std::error::Error;
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Verbosity is driven by RUST_LOG, defaulting to `info` when it is unset.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::try_default()?;
    let mut app = Application::new(config);
    app.load()?;

    tracing::info!(
        models = app.model_manager.get_models().len(),
        "Application loaded"
    );

    app.unload();
    Ok(())
}
