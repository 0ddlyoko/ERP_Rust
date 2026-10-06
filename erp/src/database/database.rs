use crate::database::FieldType;
use crate::model::ModelManager;
use erp_search::{SearchOptions, SearchType};
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
    /// The pool could not hand out a connection: none free before the timeout, or none openable.
    #[error("No database connection available: {0}")]
    Pool(String),
}

pub trait Database {
    /// Check if given database is already installed
    fn is_installed(&mut self) -> Result<bool>;

    /// Initialize this database
    fn initialize(&mut self) -> Result<()>;

    /// Bring the physical schema in line with a model's declared fields.
    ///
    /// Returns the fields it had to make room for. A column that was not there is a column with
    /// nothing in it, and a computed field that has just become stored has to be worked out for
    /// the records that already exist — nothing else would ever fill it.
    ///
    /// Only ever adds. A field that stops being stored, or stops being declared at all, keeps its
    /// column: the data in it is the user's, and a schema this decides on its own is not allowed
    /// to throw it away.
    ///
    /// A schemaless backend has nothing to do here, and nothing to report.
    fn sync_model(
        &mut self,
        _model: &erp_internal_types::FinalInternalModel,
    ) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// Make a search request to a specific model, and only return ids that match this search request
    ///
    /// ModelManager is needed to know the current structure of the database, and to make correct
    /// links between the domain and the database
    fn find_ids(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
        options: &SearchOptions,
    ) -> Result<Vec<u32>>;

    /// Count the records matching a domain.
    ///
    /// Kept apart from [`SearchOptions`] because counting happens before any limit applies, so
    /// folding the two together would be ambiguous.
    fn count(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
    ) -> Result<u32>;

    /// Make a search request to a specific model, and return ids and fields that match this search request
    ///
    /// ModelManager is needed to know the current structure of the database, and to make correct
    /// links between the domain and the database
    ///
    /// A SQL backend implements the ordering and paging with `ORDER BY` and `LIMIT` in the same
    /// statement; applying them afterwards would mean fetching every matching row first.
    fn search<'a>(
        &mut self,
        model_name: &str,
        fields: &[&'a str],
        domain: &SearchType,
        model_manager: &ModelManager,
        options: &SearchOptions,
    ) -> Result<Vec<SearchedRow<'a>>>;

    /// Create one new record per given data for given model
    fn create(&mut self, model_name: &str, data: &[&MapOfFields]) -> Result<Vec<u32>>;

    /// Update given data for given model
    fn update(&mut self, model_name: &str, data: &HashMap<u32, &MapOfFields>) -> Result<u32>;

    /// Pairs held by a relation table, for each of `ids`.
    ///
    /// A many2many lives in a table of its own rather than in a column, so it does not go
    /// through the row operations above.
    fn read_relation(
        &mut self,
        relation: &str,
        column: &str,
        target_column: &str,
        ids: &[u32],
    ) -> Result<HashMap<u32, Vec<u32>>>;

    /// Replace the pairs of one record in a relation table.
    fn write_relation(
        &mut self,
        relation: &str,
        column: &str,
        target_column: &str,
        id: u32,
        targets: &[u32],
    ) -> Result<()>;

    /// Add the constraints a model's relations need, once every table they point at exists.
    ///
    /// Separate from [`Database::sync_model`] because a many2many's relation table references
    /// two model tables, and the model declaring it is not necessarily synchronised last.
    fn sync_constraints(&mut self, _model: &erp_internal_types::FinalInternalModel) -> Result<()> {
        Ok(())
    }

    /// Index what a model's records are looked up by: what its fields ask for, its many2ones,
    /// and the far side of its many2manys.
    fn sync_indexes(&mut self, _model: &erp_internal_types::FinalInternalModel) -> Result<()> {
        Ok(())
    }

    /// Delete the given records, and return how many rows were actually removed.
    ///
    /// Ids that are not present are skipped rather than reported, mirroring `update`.
    fn delete(&mut self, model_name: &str, ids: &[u32]) -> Result<u32>;

    /// Lock the rows of these records until the transaction ends: another transaction locking
    /// them waits for this one to commit or roll back.
    ///
    /// A database that runs one transaction at a time on a row, or none, has nothing to lock.
    fn lock(&mut self, _model_name: &str, _ids: &[u32]) -> Result<()> {
        Ok(())
    }

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
