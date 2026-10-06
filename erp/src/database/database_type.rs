use crate::database::cache::CacheConnection;
use crate::database::postgres::PostgresDatabase;
use crate::database::{Database, SearchedRow};
use crate::model::ModelManager;
use erp_search::{SearchOptions, SearchType};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub enum DatabaseType {
    Cache(CacheConnection),
    Postgres(Box<PostgresDatabase>),
}

impl Database for DatabaseType {
    fn is_installed(&mut self) -> Result<bool> {
        match self {
            DatabaseType::Cache(cache) => cache.is_installed(),
            DatabaseType::Postgres(postgres) => postgres.is_installed(),
        }
    }

    fn initialize(&mut self) -> Result<()> {
        match self {
            DatabaseType::Cache(cache) => cache.initialize(),
            DatabaseType::Postgres(postgres) => postgres.initialize(),
        }
    }

    fn sync_model(
        &mut self,
        model: &erp_internal_types::FinalInternalModel,
    ) -> Result<Vec<String>> {
        match self {
            DatabaseType::Cache(cache) => cache.sync_model(model),
            DatabaseType::Postgres(postgres) => postgres.sync_model(model),
        }
    }

    fn sync_constraints(&mut self, model: &erp_internal_types::FinalInternalModel) -> Result<()> {
        match self {
            DatabaseType::Cache(cache) => cache.sync_constraints(model),
            DatabaseType::Postgres(postgres) => postgres.sync_constraints(model),
        }
    }

    fn sync_indexes(&mut self, model: &erp_internal_types::FinalInternalModel) -> Result<()> {
        match self {
            DatabaseType::Cache(cache) => cache.sync_indexes(model),
            DatabaseType::Postgres(postgres) => postgres.sync_indexes(model),
        }
    }

    fn find_ids(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
        options: &SearchOptions,
    ) -> Result<Vec<u32>> {
        match self {
            DatabaseType::Cache(cache) => {
                cache.find_ids(model_name, domain, model_manager, options)
            }
            DatabaseType::Postgres(postgres) => {
                postgres.find_ids(model_name, domain, model_manager, options)
            }
        }
    }

    fn count(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
    ) -> Result<u32> {
        match self {
            DatabaseType::Cache(cache) => cache.count(model_name, domain, model_manager),
            DatabaseType::Postgres(postgres) => postgres.count(model_name, domain, model_manager),
        }
    }

    fn search<'a>(
        &mut self,
        model_name: &str,
        fields: &[&'a str],
        domain: &SearchType,
        model_manager: &ModelManager,
        options: &SearchOptions,
    ) -> Result<Vec<SearchedRow<'a>>> {
        match self {
            DatabaseType::Cache(cache) => {
                cache.search(model_name, fields, domain, model_manager, options)
            }
            DatabaseType::Postgres(postgres) => {
                postgres.search(model_name, fields, domain, model_manager, options)
            }
        }
    }

    fn create(&mut self, model_name: &str, data: &[&MapOfFields]) -> Result<Vec<u32>> {
        match self {
            DatabaseType::Cache(cache) => cache.create(model_name, data),
            DatabaseType::Postgres(postgres) => postgres.create(model_name, data),
        }
    }

    fn update(&mut self, model_name: &str, data: &HashMap<u32, &MapOfFields>) -> Result<u32> {
        match self {
            DatabaseType::Cache(cache) => cache.update(model_name, data),
            DatabaseType::Postgres(postgres) => postgres.update(model_name, data),
        }
    }

    fn read_relation(
        &mut self,
        relation: &str,
        column: &str,
        target_column: &str,
        ids: &[u32],
    ) -> Result<HashMap<u32, Vec<u32>>> {
        match self {
            DatabaseType::Cache(cache) => cache.read_relation(relation, column, target_column, ids),
            DatabaseType::Postgres(postgres) => {
                postgres.read_relation(relation, column, target_column, ids)
            }
        }
    }

    fn write_relation(
        &mut self,
        relation: &str,
        column: &str,
        target_column: &str,
        id: u32,
        targets: &[u32],
    ) -> Result<()> {
        match self {
            DatabaseType::Cache(cache) => {
                cache.write_relation(relation, column, target_column, id, targets)
            }
            DatabaseType::Postgres(postgres) => {
                postgres.write_relation(relation, column, target_column, id, targets)
            }
        }
    }

    fn delete(&mut self, model_name: &str, ids: &[u32]) -> Result<u32> {
        match self {
            DatabaseType::Cache(cache) => cache.delete(model_name, ids),
            DatabaseType::Postgres(postgres) => postgres.delete(model_name, ids),
        }
    }

    fn lock(&mut self, model_name: &str, ids: &[u32]) -> Result<()> {
        match self {
            DatabaseType::Cache(cache) => cache.lock(model_name, ids),
            DatabaseType::Postgres(postgres) => postgres.lock(model_name, ids),
        }
    }

    fn get_installed_plugins(&mut self) -> Result<Vec<String>> {
        match self {
            DatabaseType::Cache(cache) => cache.get_installed_plugins(),
            DatabaseType::Postgres(postgres) => postgres.get_installed_plugins(),
        }
    }

    fn savepoint(&mut self, name: &str) -> Result<()> {
        match self {
            DatabaseType::Cache(cache) => cache.savepoint(name),
            DatabaseType::Postgres(postgres) => postgres.savepoint(name),
        }
    }

    fn savepoint_commit(&mut self, name: &str) -> Result<()> {
        match self {
            DatabaseType::Cache(cache) => cache.savepoint_commit(name),
            DatabaseType::Postgres(postgres) => postgres.savepoint_commit(name),
        }
    }

    fn savepoint_rollback(&mut self, name: &str) -> Result<()> {
        match self {
            DatabaseType::Cache(cache) => cache.savepoint_rollback(name),
            DatabaseType::Postgres(postgres) => postgres.savepoint_rollback(name),
        }
    }

    fn start_transaction(&mut self) -> Result<()> {
        match self {
            DatabaseType::Cache(cache) => cache.start_transaction(),
            DatabaseType::Postgres(postgres) => postgres.start_transaction(),
        }
    }

    fn commit_transaction(&mut self) -> Result<()> {
        match self {
            DatabaseType::Cache(cache) => cache.commit_transaction(),
            DatabaseType::Postgres(postgres) => postgres.commit_transaction(),
        }
    }

    fn rollback_transaction(&mut self) -> Result<()> {
        match self {
            DatabaseType::Cache(cache) => cache.rollback_transaction(),
            DatabaseType::Postgres(postgres) => postgres.rollback_transaction(),
        }
    }
}

// impl Default for DatabaseType {
//     fn default() -> Self {
//         DatabaseType::Cache(CacheDatabase::connect(&DatabaseConfig::default()).unwrap())
//     }
// }
