use std::collections::HashMap;
use std::error::Error;
use erp_search::SearchType;
use crate::database::cache::CacheDatabase;
use crate::database::{Database, DatabaseConfig, ErrorType, FieldType};
use crate::database::postgres::PostgresDatabase;
use crate::model::{MapOfFields, ModelManager};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub enum DatabaseType {
    Cache(CacheDatabase),
    Postgres(PostgresDatabase),
}

impl Database for DatabaseType {
    fn connect(_config: &DatabaseConfig) -> std::result::Result<Self, ErrorType>
    where
        Self: Sized
    {
        panic!("You should not directly call this method")
    }

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

    fn browse(&mut self, model_name: &str, domain: &SearchType, model_manager: &ModelManager) -> Result<Vec<u32>> {
        match self {
            DatabaseType::Cache(cache) => cache.browse(model_name, domain, model_manager),
            DatabaseType::Postgres(postgres) => postgres.browse(model_name, domain, model_manager),
        }
    }

    fn search<'a>(&mut self, model_name: &str, fields: &[&'a str], domain: &SearchType, model_manager: &ModelManager) -> Result<Vec<(u32, HashMap<&'a str, Option<FieldType>>)>> {
        match self {
            DatabaseType::Cache(cache) => cache.search(model_name, fields, domain, model_manager),
            DatabaseType::Postgres(postgres) => postgres.search(model_name, fields, domain, model_manager),
        }
    }

    fn create(&mut self, model_name: &str, data: &Vec<&MapOfFields>) -> Result<Vec<u32>> {
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
}

impl From<CacheDatabase> for DatabaseType {
    fn from(cache: CacheDatabase) -> Self {
        DatabaseType::Cache(cache)
    }
}

impl From<PostgresDatabase> for DatabaseType {
    fn from(postgres: PostgresDatabase) -> Self {
        DatabaseType::Postgres(postgres)
    }
}
