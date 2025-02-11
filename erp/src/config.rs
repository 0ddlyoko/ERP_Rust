use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Database {
    url: String,
    port: u32,
    name: String,
    user: String,
    password: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Config {
    database: Database,
    pub plugin_path: String,
}
