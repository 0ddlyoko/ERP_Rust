use crate::database::cache::{Row, Table};
use crate::database::{Database, DatabaseConfig, ErrorType, FieldType};
use crate::field::FieldReference;
use crate::model::{MapOfFields, ModelManager};
use erp_search::{LeftTuple, RightTuple, SearchOperator, SearchTuple, SearchType};
use std::collections::{HashMap, HashSet};
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

impl CacheDatabase {
    /// Poorly optimized search into the cache
    ///
    /// I know this method is not optimized, and I don't care as it's only used in tests
    fn get_rows(&self, model_name: &str, domain: &SearchType, model_manager: &ModelManager) -> Result<Vec<u32>> {
        Ok(match domain {
            SearchType::And(left, right) => {
                let left = self.get_rows(model_name, left, model_manager)?;
                let right = self.get_rows(model_name, right, model_manager)?;
                left.into_iter().filter(|id| right.contains(id)).collect()
            },
            SearchType::Or(left, right) => {
                let mut left = self.get_rows(model_name, left, model_manager)?;
                let mut right = self.get_rows(model_name, right, model_manager)?;
                left.append(&mut right);
                HashSet::<_>::from_iter(left).into_iter().collect()
            },
            SearchType::Tuple(SearchTuple { left: LeftTuple { path }, operator, right }) => {
                let mut path = path.clone();
                path.reverse();
                let result = self._search_path(model_name, &mut path, operator, right, model_manager);
                HashSet::<_>::from_iter(result).into_iter().collect()
            },
            SearchType::Nothing => vec![],
        })
    }

    // Path should be reverted
    fn _search_path(&self, model_name: &str, path: &mut Vec<String>, operator: &SearchOperator, right: &RightTuple, model_manager: &ModelManager) -> Vec<u32> {
        let current_field = path.pop().unwrap();
        if path.is_empty() {
            return self._get_rows(model_name, &current_field, operator, right);
        }
        let model = model_manager.get_model(model_name).unwrap_or_else(|| panic!("Model {} does not exist. This should not occur, as this is checked in method get_fields_to_save", model_name));
        let final_field = model.get_internal_field(&current_field);

        let FieldReference {target_model, inverse_field} = final_field.inverse.as_ref().unwrap_or_else(|| panic!("Field {}.{} doesn't have any inverse fields. This should not occur, as this is checked in method get_fields_to_save", model_name, current_field));
        let target_model = model_manager.get_model(target_model).unwrap_or_else(|| panic!("Model {} does not exist. This should not occur, as this is checked in method get_fields_to_save", target_model));

        let ids = self._search_path(&target_model.name, path, operator, right, model_manager);

        let ids = if matches!(final_field.default_value, crate::field::FieldType::Ref(_)) {
            self._get_rows(&model.name, &final_field.name, &SearchOperator::Equal, &ids.into())
        } else {
            let mut result: Vec<u32> = Vec::new();
            let table = self.tables.get(&target_model.name).unwrap();
            for id in ids {
                let row = table.get_row(&id).unwrap();
                if let Some(FieldType::UInteger(id)) = row.get_cell(inverse_field.as_ref().unwrap()) {
                    result.push(*id)
                }
            }
            result
        };

        ids
    }

    fn _get_rows(&self, model_name: &str, field_name: &str, operator: &SearchOperator, right: &RightTuple) -> Vec<u32> {
        let mut result = Vec::new();
        if let Some(table) = self.tables.get(model_name) {
            for (id, row) in &table.rows {
                if row.is_valid(field_name, operator, right) {
                    result.push(*id);
                }
            }
        }
        result
    }
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
    fn browse(&mut self, model_name: &str, domain: &SearchType, model_manager: &ModelManager) -> Result<Vec<u32>> {
        self.get_rows(model_name, domain, model_manager)
    }

    /// Make a search request to a specific model, and return ids and fields that match this search request
    fn search<'a>(&mut self, model_name: &str, fields: &[&'a str], domain: &SearchType, model_manager: &ModelManager) -> Result<Vec<(u32, HashMap<&'a str, Option<FieldType>>)>> {
        // We don't care about searching 2 times (one to retrieve ids and one to retrieve fields), as it's cache
        let ids = self.browse(model_name, domain, model_manager)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }
        // Following error should never occur, as if table doesn't exist then .browse should return an empty list
        let table = self.tables.get(model_name).unwrap_or_else(|| panic!("Table {model_name} should exist in cache"));
        let mut result = vec![];
        for id in ids.iter() {
            let mut fields_result = HashMap::new();
            // Following error should never occur, as ids returned by "browse" method are ids already present in table
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

    fn create(&mut self, model_name: &str, data: Vec<MapOfFields>) -> Result<Vec<u32>> {
        let table = self.tables.entry(model_name.to_string()).or_default();
        let mut ids = Vec::with_capacity(data.len());
        for d in data {
            let cells = d.fields.into_iter().map(|(k, v)| {
                let v = v.map(|value| value.into());
                (k.clone(), v)
            }).collect::<HashMap<_, _>>();
            let row = Row {
                id: 0,
                cells,
            };
            let id = table.add_row(row);
            ids.push(id);
        }
        Ok(ids)
    }

    fn update(&mut self, model_name: &str, data: &HashMap<u32, &MapOfFields>) -> Result<u32> {
        let mut number_of_updates = 0;
        if let Some(table) = self.tables.get_mut(model_name) {
            for (id, map_of_field) in data {
                if let Some(row) = table.get_row_mut(id) {
                    for (field_name, value) in &map_of_field.fields {
                        if field_name == "id" {
                            continue;
                        }
                        let result = value.as_ref().map(|field_type| field_type.clone().into());
                        row.set_cell(field_name, result);
                    }
                    number_of_updates += 1;
                }
            }
        }
        // If model not present in database, do nothing
        Ok(number_of_updates)
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
