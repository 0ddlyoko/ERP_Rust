use std::collections::HashMap;
use std::error::Error;
use erp_search::SearchType;
use crate::database::{DatabaseConfig, FieldType};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub trait Database {

    /// Make a connection to this database
    fn connect(config: &DatabaseConfig) -> std::result::Result<Self, postgres::Error>
    where
        Self: Sized;

    /// Check if given database is already installed
    fn is_installed(&mut self) -> Result<bool>;

    /// Initialize this database
    fn initialize(&mut self) -> Result<()>;

    /// Make a search request to a specific model, and only return ids that match this search request
    fn browse(&mut self, model_name: &str, domain: &SearchType) -> Result<Vec<u32>>;

    /// Make a search request to a specific model, and return ids and fields that match this search request
    fn search<'a>(&mut self, model_name: &str, fields: Vec<&'a str>, domain: &SearchType) -> Result<Vec<(u32, HashMap<&'a str, Option<FieldType>>)>>;

    /// Retrieves installed plugins
    fn get_installed_plugins(&mut self) -> Result<Vec<String>>;
}
