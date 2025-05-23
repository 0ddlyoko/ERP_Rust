use crate::database::cache::Table;
use crate::database::{Database, DatabaseConfig, ErrorType, FieldType};
use erp_search::SearchType;
use std::collections::HashMap;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// In-memory database, used mainly for testing.
///
/// This cache database could also be used in the app
pub struct CacheDatabase {
    installed: bool,
    tables: HashMap<String, Table>,
    savepoints: Vec<(String, HashMap<String, Table>)>,
}

impl Database for CacheDatabase {

    /// Make a connection to this database
    fn connect(_config: &DatabaseConfig) -> std::result::Result<Self, ErrorType>
    where
        Self: Sized
    {
        Ok(Self {
            installed: false,
            tables: HashMap::new(),
            savepoints: Vec::new(),
        })
    }

    /// Check if given database is already installed
    fn is_installed(&mut self) -> Result<bool> {
        Ok(self.installed)
    }

    /// Initialize this database
    fn initialize(&mut self) -> Result<()> {
        self.installed = true;
        Ok(())
    }

    /// Make a search request to a specific model, and only return ids that match this search request
    fn browse(&mut self, model_name: &str, domain: &SearchType) -> Result<Vec<u32>> {
        if let Some(table) = self.tables.get(model_name) {
            let mut result = vec![];
            for (id, row) in table.rows.iter() {
                if row.is_valid(domain) {
                    result.push(*id);
                }
            }
            Ok(result)
        } else {
            Ok(vec![])
        }
    }

    /// Make a search request to a specific model, and return ids and fields that match this search request
    fn search<'a>(&mut self, model_name: &str, fields: &[&'a str], domain: &SearchType) -> Result<Vec<(u32, HashMap<&'a str, Option<FieldType>>)>> {
        let ids = self.browse(model_name, domain)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let table = self.tables.get(model_name).unwrap_or_else(|| panic!("Table {model_name} should exist in cache"));
        let mut result = vec![];
        for id in ids.iter() {
            let mut fields_result = HashMap::new();
            let row = table.get_row(id).unwrap_or_else(|| panic!("Row with id {id} in table {model_name} should exist in cache"));
            for field_name in fields.iter() {
                if field_name == &"id" {
                    fields_result.insert(*field_name, Some(FieldType::UInteger(*id)));
                    continue;
                }
                fields_result.insert(*field_name, row.get_cell(field_name).clone());
            }
            result.push((*id, fields_result));
        }
        Ok(result)
    }

    fn get_installed_plugins(&mut self) -> Result<Vec<String>> {
        if !self.installed {
            return Ok(vec![]);
        }
        let table = self.tables.get("plugin");
        if let Some(table) = table {
            let mut result = vec![];
            for row in table.rows.values() {
                let cell_state = row.get_cell("state");
                let cell_name = row.get_cell("name");
                match (cell_state, cell_name) {
                    (Some(FieldType::String(state)), Some(FieldType::String(name))) if state == "installed" => {
                        result.push(name.clone());
                    },
                    _ => {}
                }
            }
            Ok(result)
        } else {
            Ok(vec![])
        }
    }

    fn savepoint(&mut self, name: &str) -> Result<()> {
        let tables = self.tables.clone();
        self.savepoints.push((name.to_string(), tables));
        Ok(())
    }

    fn savepoint_commit(&mut self, name: &str) -> Result<()> {
        // TODO Create real errors
        if let Some((savepoint_name, _map)) = self.savepoints.last() {
            if savepoint_name == name {
                self.savepoints.pop();
                Ok(())
            } else {
                Err(format!("Last savepoint is not {name}").into())
            }
        } else {
            Err("Cannot commit a missing savepoint".into())
        }
    }

    fn savepoint_rollback(&mut self, name: &str) -> Result<()> {
        if let Some((savepoint_name, _map)) = self.savepoints.last() {
            if savepoint_name == name {
                let (_savepoint_name, map) = self.savepoints.pop().unwrap();
                self.tables = map;
                Ok(())
            } else {
                Err(format!("Last savepoint is not {name}").into())
            }
        } else {
            Err("Cannot commit a missing savepoint".into())
        }
    }
}
