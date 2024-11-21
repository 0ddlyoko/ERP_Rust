use ::config::ConfigError;

pub mod config;
pub mod models;

pub fn default_config() -> Result<erp::config::Config, ConfigError> {
    config::build_config()
}
