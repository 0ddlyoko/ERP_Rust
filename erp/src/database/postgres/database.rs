use crate::database::{Database, DatabaseConfig, ErrorType, FieldType};
use crate::model::{MapOfFields, ModelManager};
use erp_search::SearchType;
use postgres::{Client, NoTls};
use std::collections::HashMap;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub struct PostgresDatabase {
    pub client: Client,
    schema: String,
    is_transaction: bool,
}

impl Database for PostgresDatabase {

    /// Make a connection to this database
    fn connect(config: &DatabaseConfig) -> std::result::Result<Self, ErrorType>
    where
        Self: Sized
    {
        let user = &config.user;
        let password = &config.password;
        let url = &config.url;
        let db_name = &config.name;
        let schema = &config.schema;
        let connect = format!("postgresql://{user}:{password}@{url}/{db_name}");
        let client = Client::connect(connect.as_str(), NoTls)?;
        Ok(Self {
            client,
            schema: schema.clone(),
            is_transaction: false,
        })
    }

    /// Check if given database is already installed
    fn is_installed(&mut self) -> Result<bool> {
        let result = self.client.query_one("SELECT EXISTS (
            SELECT FROM \"pg_tables\" WHERE \"schemaname\"=$1 AND \"tablename\"='plugin'
        )", &[&self.schema])?;
        Ok(result.try_get(0)?)
    }

    /// Initialize this database
    fn initialize(&mut self) -> Result<()> {
        // TODO Put this in a file
        self.client.batch_execute("
            CREATE TABLE plugin (
                id              SERIAL PRIMARY KEY,
                name            VARCHAR NOT NULL,
                description     TEXT,
                website         TEXT,
                url             TEXT,
                state           VARCHAR NOT NULL
            )
            ")?;
        Ok(())
    }

    /// Make a search request to a specific model, and only return ids that match this search request
    fn browse(&mut self, _model_name: &str, _domain: &SearchType, _model_manager: &ModelManager) -> Result<Vec<u32>> {
        todo!()
    }

    /// Make a search request to a specific model, and return ids and fields that match this search request
    fn search<'a>(&mut self, _model_name: &str, _fields: &[&'a str], _domain: &SearchType, _model_manager: &ModelManager) -> Result<Vec<(u32, HashMap<&'a str, Option<FieldType>>)>> {
        todo!()
    }

    fn create(&mut self, _model_name: &str, _data: &Vec<&MapOfFields>) -> Result<Vec<u32>> {
        todo!()
    }

    fn update(&mut self, _model_name: &str, _data: &HashMap<u32, &MapOfFields>) -> Result<u32> {
        todo!()
    }

    fn get_installed_plugins(&mut self) -> Result<Vec<String>> {
        let mut result = vec![];
        for row in self.client.query("SELECT \"name\" FROM \"plugin\" WHERE \"state\"=\'installed\'", &[])? {
            let name: &str = row.get(0);
            result.push(name.to_string());
        }
        Ok(result)
    }

    fn savepoint(&mut self, name: &str) -> Result<()> {
        Ok(self.client.batch_execute(&format!("SAVEPOINT {}", name))?)
    }

    fn savepoint_commit(&mut self, name: &str) -> Result<()> {
        Ok(self.client.batch_execute(&format!("RELEASE {}", name))?)
    }

    fn savepoint_rollback(&mut self, name: &str) -> Result<()> {
        Ok(self.client.batch_execute(&format!("ROLLBACK TO {}", name))?)
    }

    fn start_transaction(&mut self) -> Result<()> {
        self.is_transaction = true;
        Ok(self.client.batch_execute("START TRANSACTION")?)
    }

    fn commit_transaction(&mut self) -> Result<()> {
        self.is_transaction = false;
        Ok(self.client.batch_execute("COMMIT")?)
    }

    fn rollback_transaction(&mut self) -> Result<()> {
        self.is_transaction = false;
        Ok(self.client.batch_execute("ROLLBACK")?)
    }
}

impl Drop for PostgresDatabase {
    fn drop(&mut self) {
        // Rollback if needed
        if self.is_transaction {
            self.rollback_transaction();
        }
    }
}
