use serde::Deserialize;

fn default_port() -> u16 {
    5432
}

/// Fields are public so an embedder — or a test — can configure the database in code
/// instead of through a file.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub name: String,
    pub schema: String,
    pub user: String,
    pub password: String,
}
