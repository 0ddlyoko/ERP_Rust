use crate::database::{DatabaseConfig, FieldType};
use erp_search::SearchType;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use crate::model::MapOfFields;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub enum ErrorType {
    Postgres(postgres::Error),
    Other(Box<dyn Error>),
}

impl From<postgres::Error> for ErrorType {
    fn from(e: postgres::Error) -> Self {
        ErrorType::Postgres(e)
    }
}

impl From<Box<dyn Error>> for ErrorType {
    fn from(e: Box<dyn Error>) -> Self {
        ErrorType::Other(e)
    }
}

impl Display for ErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ErrorType::Postgres(e) => e.fmt(f),
            ErrorType::Other(e) => e.fmt(f),
        }
    }
}

impl Error for ErrorType {}


pub trait Database {

    /// Make a connection to this database
    fn connect(config: &DatabaseConfig) -> std::result::Result<Self, ErrorType>
    where
        Self: Sized;

    /// Check if given database is already installed
    fn is_installed(&mut self) -> Result<bool>;

    /// Initialize this database
    fn initialize(&mut self) -> Result<()>;

    /// Make a search request to a specific model, and only return ids that match this search request
    fn browse(&mut self, model_name: &str, domain: &SearchType) -> Result<Vec<u32>>;

    /// Make a search request to a specific model, and return ids and fields that match this search request
    fn search<'a>(&mut self, model_name: &str, fields: &[&'a str], domain: &SearchType) -> Result<Vec<(u32, HashMap<&'a str, Option<FieldType>>)>>;

    /// Create one new record per given data for given model
    fn create(&mut self, model_name: &str, data: &[&MapOfFields]) -> Result<Vec<u32>>;

    /// Update given data for given model
    fn update(&mut self, model_name: &str, data: &HashMap<u32, &MapOfFields>) -> Result<u32>;

    /// Retrieves installed plugins
    fn get_installed_plugins(&mut self) -> Result<Vec<String>>;
    
    /// Create a new savepoint
    fn savepoint(&mut self, name: &str) -> Result<()>;
    
    /// Commit previously created savepoint
    fn savepoint_commit(&mut self, name: &str) -> Result<()>;
    
    /// Rollback previously created savepoint
    fn savepoint_rollback(&mut self, name: &str) -> Result<()>;
}
