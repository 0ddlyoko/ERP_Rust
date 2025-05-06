use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
#[allow(dead_code)]
pub struct DatabaseConfig {
    pub(crate) url: String,
    pub(crate) name: String,
    pub(crate) schema: String,
    pub(crate) user: String,
    pub(crate) password: String,
}