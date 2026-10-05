use crate::database::DatabaseConfig;
use crate::server_config::ServerConfig;
use config::ConfigError;
use directories::ProjectDirs;
use serde_derive::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Default, Clone)]
#[allow(dead_code)]
pub struct Config {
    pub database: DatabaseConfig,
    #[serde(default)]
    pub server: ServerConfig,
    pub plugin_path: String,
}

impl Config {
    pub fn try_default() -> Result<Config, ConfigError> {
        let Some(config_dir) = ProjectDirs::from("me", "oddlyoko", "erp") else {
            return Err(ConfigError::Message(
                "Cannot determine the user configuration directory".to_string(),
            ));
        };
        let config_file = Path::join(config_dir.config_dir(), "config.toml");

        tracing::info!(?config_file, "Loading config");
        let config = config::Config::builder()
            .set_default("database.url", "localhost")?
            .set_default("database.name", "erp")?
            .set_default("database.schema", "public")?
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 8080)?
            .add_source(config::File::from(config_file).required(true))
            .add_source(
                config::Environment::with_prefix("ERP")
                    .try_parsing(true)
                    .separator("_"),
            )
            .build()?;

        config.try_deserialize()
    }
}
