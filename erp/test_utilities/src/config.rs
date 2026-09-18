use std::path::Path;

use config::{Config, ConfigError, Environment, File};
use directories::ProjectDirs;

pub(crate) fn build_config() -> Result<erp::config::Config, ConfigError> {
    let Some(config_dir) = ProjectDirs::from("me", "oddlyoko", "erp") else {
        return Err(ConfigError::Message(
            "Cannot determine the user configuration directory".to_string(),
        ));
    };
    let config_file = Path::join(config_dir.config_dir(), "config.toml");
    let config_test_file = Path::join(config_dir.config_dir(), "config_test.toml");

    let config = Config::builder()
        .set_default("database.url", "localhost")?
        .set_default("database.port", 5432)?
        .set_default("database.name", "erp")?
        .add_source(File::from(config_file).required(false))
        .add_source(File::from(config_test_file).required(false))
        .add_source(
            Environment::with_prefix("ERP")
                .try_parsing(true)
                .separator("_")
                .list_separator(" "),
        )
        .build()?;

    config.try_deserialize()
}
