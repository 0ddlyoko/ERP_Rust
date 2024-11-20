use ::config::ConfigError;

pub mod config;
pub mod models;

pub fn default_config() -> Result<core::config::Config, ConfigError> {
    config::build_config()
}
