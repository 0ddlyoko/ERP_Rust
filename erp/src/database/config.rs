use serde::Deserialize;

fn default_port() -> u16 {
    5432
}

fn default_pool_size() -> u32 {
    10
}

fn default_connection_timeout() -> u64 {
    30
}

/// Fields are public so an embedder — or a test — can configure the database in code
/// instead of through a file.
#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub name: String,
    pub schema: String,
    pub user: String,
    pub password: String,
    /// Connections kept open. Every environment takes one for as long as its transaction lasts,
    /// so this is the ceiling on transactions running at once.
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    /// How long to wait for a free connection before giving up, in seconds.
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            port: default_port(),
            name: String::new(),
            schema: String::new(),
            user: String::new(),
            password: String::new(),
            pool_size: default_pool_size(),
            connection_timeout: default_connection_timeout(),
        }
    }
}
