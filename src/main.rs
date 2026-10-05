use erp::app::{Application, LaunchArgs};
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
    let launch = LaunchArgs::from_args(std::env::args().skip(1))?;
    app.set_install(launch.install);
    app.set_data_update(launch.update);
    app.load()?;

    tracing::info!(
        models = app.model_manager.get_models().len(),
        "Application loaded"
    );

    app.unload();
    Ok(())
}
