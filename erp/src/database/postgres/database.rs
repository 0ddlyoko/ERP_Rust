use std::collections::HashMap;
use erp_search::SearchType;
use std::error::Error;
use postgres::{Client, NoTls};
use crate::database::{Database, DatabaseConfig, ErrorType, FieldType};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub struct PostgresDatabase {
    client: Client,
    schema: String,
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
    fn browse(&mut self, model_name: &str, domain: &SearchType) -> Result<Vec<u32>> {
        todo!()
    }

    /// Make a search request to a specific model, and return ids and fields that match this search request
    fn search<'a>(&mut self, model_name: &str, fields: Vec<&'a str>, domain: &SearchType) -> Result<Vec<(u32, HashMap<&'a str, Option<FieldType>>)>> {
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
}
