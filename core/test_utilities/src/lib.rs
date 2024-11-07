use ::config::ConfigError;

pub mod config;

pub fn default_config() -> Result<core::config::Config, ConfigError> {
    config::build_config()
}
