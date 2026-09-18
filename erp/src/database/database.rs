use crate::database::FieldType;
use crate::model::ModelManager;
use erp_search::SearchType;
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// One row returned by a search: its id, and the value read for each requested field.
pub type SearchedRow<'a> = (u32, HashMap<&'a str, Option<FieldType>>);

#[derive(Debug, thiserror::Error)]
pub enum ErrorType {
    #[error(transparent)]
    Postgres(#[from] postgres::Error),
    #[error(transparent)]
    Other(#[from] Box<dyn Error + Send + Sync>),
}

pub trait Database {
    /// Check if given database is already installed
    fn is_installed(&mut self) -> Result<bool>;

    /// Initialize this database
    fn initialize(&mut self) -> Result<()>;

    /// Make a search request to a specific model, and only return ids that match this search request
    ///
    /// ModelManager is needed to know the current structure of the database, and to make correct
    /// links between the domain and the database
    fn browse(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
    ) -> Result<Vec<u32>>;

    /// Make a search request to a specific model, and return ids and fields that match this search request
    ///
    /// ModelManager is needed to know the current structure of the database, and to make correct
    /// links between the domain and the database
    fn search<'a>(
        &mut self,
        model_name: &str,
        fields: &[&'a str],
        domain: &SearchType,
        model_manager: &ModelManager,
    ) -> Result<Vec<SearchedRow<'a>>>;

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

    /// Start a new transaction
    ///
    /// This method should be called before creating any savepoint
    fn start_transaction(&mut self) -> Result<()>;

    /// Commit every request made from the creation of the latest transaction
    fn commit_transaction(&mut self) -> Result<()>;

    /// Rollback every request made from the creation of the latest transaction
    fn rollback_transaction(&mut self) -> Result<()>;
}
