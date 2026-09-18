use crate::database::cache::{Row, Table};
use crate::database::{Database, FieldType};
use crate::model::ModelManager;
use erp_search::{LeftTuple, RightTuple, SearchOperator, SearchTuple, SearchType};
use erp_types::field::{FieldReference, FieldReferenceType};
use erp_types::model::MapOfFields;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::{Arc, Mutex};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Committed state of the in-memory database, shared by every connection opened on it.
///
/// Also owns id allocation, so rows created concurrently by different connections never collide.
#[derive(Default)]
struct CacheStore {
    installed: bool,
    tables: HashMap<String, Table>,
    next_ids: HashMap<String, u32>,
}

impl CacheStore {
    /// Reserve `count` ids for `model_name`, so no other connection can hand out the same ones.
    fn reserve_ids(&mut self, model_name: &str, count: usize) -> Vec<u32> {
        let next_id = self.next_ids.entry(model_name.to_string()).or_insert(0);
        let first = *next_id + 1;
        *next_id += count as u32;
        (first..=*next_id).collect()
    }
}

/// A connection's savepoint: the working copy and write set to restore when rolling back to it.
struct Savepoint {
    name: Option<String>,
    tables: HashMap<String, Table>,
    written_rows: HashMap<String, HashSet<u32>>,
}

/// In-memory database, used mainly for testing.
///
/// Holds committed state only. Each [`CacheDatabase::connect`] hands out an independent
/// connection carrying its own transaction, so several environments can run side by side the way
/// they would against a real server.
#[derive(Clone, Default)]
pub struct CacheDatabase {
    store: Arc<Mutex<CacheStore>>,
}

impl CacheDatabase {
    /// Open an independent connection to this database.
    pub fn connect(&self) -> CacheConnection {
        let mut connection = CacheConnection {
            store: Arc::clone(&self.store),
            installed: false,
            tables: HashMap::new(),
            written_rows: HashMap::new(),
            savepoints: Vec::new(),
        };
        connection.reload_from_store();
        connection
    }
}

/// A single connection to a [`CacheDatabase`].
///
/// Reads and writes target a working copy taken when the transaction starts; committing publishes
/// it back to the shared store. Each connection therefore gets the isolation a real transaction
/// provides, where the database previously carried one global savepoint stack that concurrent
/// environments would have corrupted.
pub struct CacheConnection {
    store: Arc<Mutex<CacheStore>>,
    installed: bool,
    tables: HashMap<String, Table>,
    /// Rows this transaction created or updated, published row by row on commit.
    written_rows: HashMap<String, HashSet<u32>>,
    savepoints: Vec<Savepoint>,
}

impl CacheConnection {
    /// Take a fresh working copy from the shared store, discarding uncommitted changes.
    fn reload_from_store(&mut self) {
        {
            let store = self.store.lock().expect("cache database mutex poisoned");
            self.installed = store.installed;
            self.tables = store.tables.clone();
        }
        self.written_rows.clear();
    }

    /// Publish this transaction's write set to the shared store.
    ///
    /// Only the rows this connection touched are copied over, so committing never reverts rows
    /// another connection committed in the meantime.
    fn publish_to_store(&mut self) {
        {
            let mut store = self.store.lock().expect("cache database mutex poisoned");
            store.installed = self.installed;
            for (model_name, ids) in &self.written_rows {
                let Some(source) = self.tables.get(model_name) else {
                    continue;
                };
                let target = store.tables.entry(model_name.clone()).or_default();
                for id in ids {
                    if let Some(row) = source.get_row(id) {
                        target.insert_row(*id, row.clone());
                    }
                }
            }
        }
        self.written_rows.clear();
    }

    /// Record that `ids` of `model_name` were written by this transaction.
    fn mark_written(&mut self, model_name: &str, ids: impl IntoIterator<Item = u32>) {
        self.written_rows
            .entry(model_name.to_string())
            .or_default()
            .extend(ids);
    }

    /// Poorly optimized search into the cache
    ///
    /// I know this method is not optimized, and I don't care as it's only used in tests
    fn get_rows(
        &self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
    ) -> Result<Vec<u32>> {
        Ok(match domain {
            SearchType::And(left, right) => {
                let left = self.get_rows(model_name, left, model_manager)?;
                let right = self.get_rows(model_name, right, model_manager)?;
                left.into_iter().filter(|id| right.contains(id)).collect()
            }
            SearchType::Or(left, right) => {
                let mut left = self.get_rows(model_name, left, model_manager)?;
                let mut right = self.get_rows(model_name, right, model_manager)?;
                left.append(&mut right);
                HashSet::<_>::from_iter(left).into_iter().collect()
            }
            SearchType::Tuple(SearchTuple {
                left: LeftTuple { path },
                operator,
                right,
            }) => {
                let mut path = path.clone();
                path.reverse();
                let result =
                    self._search_path(model_name, &mut path, operator, right, model_manager);
                HashSet::<_>::from_iter(result).into_iter().collect()
            }
            SearchType::Nothing => vec![],
        })
    }

    // Path should be reverted
    fn _search_path(
        &self,
        model_name: &str,
        path: &mut Vec<String>,
        operator: &SearchOperator,
        right: &RightTuple,
        model_manager: &ModelManager,
    ) -> Vec<u32> {
        let current_field = path.pop().unwrap();
        if path.is_empty() {
            return self._get_rows(model_name, &current_field, operator, right);
        }
        let model = model_manager.get_model(model_name);
        let final_field = model.get_internal_field(&current_field);

        let FieldReference { target_model, inverse_field } = final_field.inverse.as_ref().unwrap_or_else(|| panic!("Field {model_name}.{current_field} doesn't have any inverse fields. This should not occur, as this is checked in method get_fields_to_save"));
        let target_model = model_manager.get_model(target_model);

        let ids = self._search_path(&target_model.name, path, operator, right, model_manager);

        let ids = if matches!(
            final_field.default_value,
            erp_types::field::FieldType::Ref(_)
        ) {
            self._get_rows(
                &model.name,
                &final_field.name,
                &SearchOperator::Equal,
                &ids.into(),
            )
        } else {
            let mut result: Vec<u32> = Vec::new();
            let table = self.tables.get(&target_model.name).unwrap();
            if let FieldReferenceType::O2M { inverse_field } = inverse_field {
                for id in ids {
                    let row = table.get_row(&id).unwrap();
                    if let Some(FieldType::UInteger(id)) = row.get_cell(inverse_field) {
                        result.push(*id)
                    }
                }
                result
            } else {
                panic!(
                    "Field {}.{} is of type M2O. This should not be possible here",
                    target_model.name, current_field
                )
            }
        };

        ids
    }

    fn _get_rows(
        &self,
        model_name: &str,
        field_name: &str,
        operator: &SearchOperator,
        right: &RightTuple,
    ) -> Vec<u32> {
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

impl Database for CacheConnection {
    /// Check if given database is already installed
    fn is_installed(&mut self) -> Result<bool> {
        let store = self.store.lock().expect("cache database mutex poisoned");
        Ok(store.installed)
    }

    /// Initialize this database.
    ///
    /// Written straight through to the shared store: installation happens at boot, outside any
    /// transaction.
    fn initialize(&mut self) -> Result<()> {
        self.installed = true;
        let mut store = self.store.lock().expect("cache database mutex poisoned");
        store.installed = true;
        Ok(())
    }

    /// Make a search request to a specific model, and only return ids that match this search request
    fn browse(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
    ) -> Result<Vec<u32>> {
        self.get_rows(model_name, domain, model_manager)
    }

    /// Make a search request to a specific model, and return ids and fields that match this search request
    fn search<'a>(
        &mut self,
        model_name: &str,
        fields: &[&'a str],
        domain: &SearchType,
        model_manager: &ModelManager,
    ) -> Result<Vec<(u32, HashMap<&'a str, Option<FieldType>>)>> {
        // We don't care about searching 2 times (one to retrieve ids and one to retrieve fields), as it's cache
        let ids = self.browse(model_name, domain, model_manager)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }
        // Following error should never occur, as if table doesn't exist then .browse should return an empty list
        let table = self
            .tables
            .get(model_name)
            .unwrap_or_else(|| panic!("Table {model_name} should exist in cache"));
        let mut result = vec![];
        for id in ids.iter() {
            let mut fields_result = HashMap::new();
            // Following error should never occur, as ids returned by "browse" method are ids already present in table
            let row = table.get_row(id).unwrap_or_else(|| {
                panic!("Row with id {id} in table {model_name} should exist in cache")
            });
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

    fn create(&mut self, model_name: &str, data: &Vec<&MapOfFields>) -> Result<Vec<u32>> {
        let ids = {
            let mut store = self.store.lock().expect("cache database mutex poisoned");
            store.reserve_ids(model_name, data.len())
        };
        let table = self.tables.entry(model_name.to_string()).or_default();
        for (id, d) in ids.iter().zip(data) {
            let cells = d
                .fields
                .iter()
                .map(|(k, v)| {
                    let v = v.clone().map(|value| value.into());
                    (k.clone(), v)
                })
                .collect::<HashMap<_, _>>();
            table.insert_row(*id, Row { id: *id, cells });
        }
        self.mark_written(model_name, ids.iter().copied());
        Ok(ids)
    }

    fn update(&mut self, model_name: &str, data: &HashMap<u32, &MapOfFields>) -> Result<u32> {
        let mut number_of_updates = 0;
        let mut updated_ids = Vec::new();
        if let Some(table) = self.tables.get_mut(model_name) {
            for (id, map_of_field) in data {
                if let Some(row) = table.get_row_mut(id) {
                    updated_ids.push(*id);
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
        self.mark_written(model_name, updated_ids);
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
                    (Some(FieldType::String(state)), Some(FieldType::String(name)))
                        if state == "installed" =>
                    {
                        result.push(name.clone());
                    }
                    _ => {}
                }
            }
            Ok(result)
        } else {
            Ok(vec![])
        }
    }

    fn savepoint(&mut self, name: &str) -> Result<()> {
        self.savepoints.push(Savepoint {
            name: Some(name.to_string()),
            tables: self.tables.clone(),
            written_rows: self.written_rows.clone(),
        });
        Ok(())
    }

    fn savepoint_commit(&mut self, name: &str) -> Result<()> {
        // TODO Create real errors
        match self.savepoints.last() {
            Some(Savepoint { name: Some(last), .. }) if last == name => {
                self.savepoints.pop();
                Ok(())
            }
            Some(_) => Err(format!("Last savepoint is not {name}").into()),
            None => Err("Cannot commit a missing savepoint".into()),
        }
    }

    fn savepoint_rollback(&mut self, name: &str) -> Result<()> {
        match self.savepoints.last() {
            Some(Savepoint { name: Some(last), .. }) if last == name => {
                let savepoint = self.savepoints.pop().expect("checked just above");
                self.tables = savepoint.tables;
                self.written_rows = savepoint.written_rows;
                Ok(())
            }
            Some(_) => Err(format!("Last savepoint is not {name}").into()),
            None => Err("Cannot roll back a missing savepoint".into()),
        }
    }

    /// Start a transaction on this connection.
    ///
    /// Refreshes the working copy first, so the transaction observes everything other connections
    /// have committed so far.
    fn start_transaction(&mut self) -> Result<()> {
        self.reload_from_store();
        self.savepoints.push(Savepoint {
            name: None,
            tables: self.tables.clone(),
            written_rows: HashMap::new(),
        });
        Ok(())
    }

    fn commit_transaction(&mut self) -> Result<()> {
        while let Some(savepoint) = self.savepoints.pop() {
            if savepoint.name.is_none() {
                self.publish_to_store();
                return Ok(());
            }
        }
        Err("No transaction to commit".into())
    }

    fn rollback_transaction(&mut self) -> Result<()> {
        while let Some(savepoint) = self.savepoints.pop() {
            if savepoint.name.is_none() {
                self.tables = savepoint.tables;
                self.written_rows = savepoint.written_rows;
                return Ok(());
            }
        }
        Err("No transaction to roll back".into())
    }
}
