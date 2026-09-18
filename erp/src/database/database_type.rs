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

    fn browse(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
        options: &SearchOptions,
    ) -> Result<Vec<u32>> {
        match self {
            DatabaseType::Cache(cache) => cache.browse(model_name, domain, model_manager, options),
            DatabaseType::Postgres(postgres) => {
                postgres.browse(model_name, domain, model_manager, options)
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

    fn delete(&mut self, model_name: &str, ids: &[u32]) -> Result<u32> {
        match self {
            DatabaseType::Cache(cache) => cache.delete(model_name, ids),
            DatabaseType::Postgres(postgres) => postgres.delete(model_name, ids),
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
