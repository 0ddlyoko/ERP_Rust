use serde::Deserialize;

fn default_port() -> u16 {
    5432
}

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct DatabaseConfig {
    pub(crate) url: String,
    #[serde(default = "default_port")]
    pub(crate) port: u16,
    pub(crate) name: String,
    pub(crate) schema: String,
    pub(crate) user: String,
    pub(crate) password: String,
}
