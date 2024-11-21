use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Database {
    url: String,
    port: u32,
    name: String,
    user: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    database: Database,
    pub plugin_path: String,
}
