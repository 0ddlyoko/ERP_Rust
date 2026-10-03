use crate::database::cache::{CacheDatabaseError, Row, Table};
use crate::database::{Database, FieldType, SearchedRow};
use crate::model::ModelManager;
use erp_search::{LeftTuple, RightTuple, SearchOperator, SearchOptions, SearchTuple, SearchType};
use erp_types::field::{FieldReference, FieldReferenceType};
use erp_types::model::MapOfFields;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::{Arc, Mutex};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

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

/// What a transaction did to a row, as it will be replayed onto the shared store.
///
/// One entry per row, so a row created then deleted in the same transaction resolves to a single
/// outcome instead of landing in two competing sets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RowOp {
    Written,
    Deleted,
}

/// The rows a transaction touched, and what it did to each.
type WriteSet = HashMap<String, HashMap<u32, RowOp>>;

/// A connection's savepoint: the working copy and write set to restore when rolling back to it.
struct Savepoint {
    name: Option<String>,
    tables: HashMap<String, Table>,
    written_rows: WriteSet,
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
    /// Rows this transaction touched, replayed row by row on commit.
    written_rows: WriteSet,
    savepoints: Vec<Savepoint>,
}

impl CacheConnection {
    /// Take a fresh working copy from the shared store, discarding uncommitted changes.
    fn reload_from_store(&mut self) {
        {
            let store = self
                .store
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.installed = store.installed;
            self.tables = store.tables.clone();
        }
        self.written_rows.clear();
    }

    /// Publish this transaction's write set to the shared store.
    ///
    /// Only the rows this connection touched are replayed, so committing never reverts rows
    /// another connection committed in the meantime. A copy alone cannot express a removal, which
    /// is why the write set records the operation and not just the id.
    fn publish_to_store(&mut self) {
        {
            let mut store = self
                .store
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            store.installed = self.installed;
            for (model_name, ops) in &self.written_rows {
                let target = store.tables.entry(model_name.clone()).or_default();
                for (id, op) in ops {
                    match op {
                        RowOp::Written => {
                            if let Some(source) = self.tables.get(model_name)
                                && let Some(row) = source.get_row(id)
                            {
                                target.insert_row(*id, row.clone());
                            }
                        }
                        RowOp::Deleted => {
                            target.delete_row(id);
                        }
                    }
                }
            }
        }
        self.written_rows.clear();
    }

    /// Record what this transaction did to `ids` of `model_name`.
    fn mark_rows(&mut self, model_name: &str, ids: impl IntoIterator<Item = u32>, op: RowOp) {
        let entry = self.written_rows.entry(model_name.to_string()).or_default();
        for id in ids {
            entry.insert(id, op);
        }
    }

    /// Sort ids on the requested keys.
    ///
    /// Keys are read once per record rather than on every comparison, and ties fall back to the
    /// id so the result stays stable.
    fn sort_ids(&self, model_name: &str, ids: &mut [u32], options: &SearchOptions) {
        let table = self.tables.get(model_name);
        let keys: HashMap<u32, Vec<Option<FieldType>>> = ids
            .iter()
            .map(|id| {
                let row = table.and_then(|table| table.get_row(id));
                let values = options
                    .order
                    .iter()
                    .map(|order| {
                        row.map(|row| row.get_cell(&order.field).clone())
                            .unwrap_or(None)
                    })
                    .collect();
                (*id, values)
            })
            .collect();

        ids.sort_by(|left, right| {
            for (index, order) in options.order.iter().enumerate() {
                let ordering = Row::compare_cells(&keys[left][index], &keys[right][index]);
                let ordering = if order.descending {
                    ordering.reverse()
                } else {
                    ordering
                };
                if ordering != std::cmp::Ordering::Equal {
                    return ordering;
                }
            }
            left.cmp(right)
        });
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
                    self._search_path(model_name, &mut path, operator, right, model_manager)?;
                HashSet::<_>::from_iter(result).into_iter().collect()
            }
            // A domain nothing satisfies, which selects no record.
            SearchType::Never => Vec::new(),
            // An empty domain filters nothing, so it selects every record. Without this there
            // is no way to express "list them all", which is what any list view starts from.
            SearchType::Nothing => self
                .tables
                .get(model_name)
                .map(|table| table.rows.keys().copied().collect())
                .unwrap_or_default(),
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
    ) -> Result<Vec<u32>> {
        let current_field = path.pop().unwrap();
        if path.is_empty() {
            let model = model_manager.get_model(model_name);
            if current_field != "id"
                && let Ok(field) = model.try_get_internal_field(&current_field)
                && let Some(reference) = &field.inverse
                && let Some(ids) = self._relation_rows(model_name, reference, operator, right)?
            {
                return Ok(ids);
            }
            return Ok(self._get_rows(model_name, &current_field, operator, right));
        }
        let model = model_manager.get_model(model_name);
        let final_field = model.get_internal_field(&current_field);

        let FieldReference { target_model, inverse_field } = final_field.inverse.as_ref().unwrap_or_else(|| panic!("Field {model_name}.{current_field} doesn't have any inverse fields. This should not occur, as this is checked in method get_fields_to_save"));
        let target_model = model_manager.get_model(target_model);

        let ids = self._search_path(&target_model.name, path, operator, right, model_manager)?;

        if final_field.kind == erp_types::field::FieldKind::Ref {
            Ok(self._get_rows(
                &model.name,
                &final_field.name,
                &SearchOperator::Equal,
                &ids.into(),
            ))
        } else {
            let mut result: Vec<u32> = Vec::new();
            // Both lookups below tolerate a missing row: deleting a record leaves the ids of
            // rows that pointed at it behind, and a search must not surface — or trip over —
            // something that is gone.
            let Some(table) = self.tables.get(&target_model.name) else {
                return Ok(result);
            };
            let owner_table = self.tables.get(model_name);
            if let FieldReferenceType::O2M { inverse_field } = inverse_field {
                for id in ids {
                    let Some(row) = table.get_row(&id) else {
                        continue;
                    };
                    if let Some(FieldType::UInteger(id)) = row.get_cell(inverse_field)
                        && owner_table.is_some_and(|table| table.get_row(id).is_some())
                    {
                        result.push(*id)
                    }
                }
                Ok(result)
            } else if let FieldReferenceType::M2M { .. } = inverse_field {
                // The records linked to any of the targets found, through the table of pairs.
                Ok(self
                    ._relation_rows(
                        model_name,
                        final_field.inverse.as_ref().expect("matched above"),
                        &SearchOperator::In,
                        &ids.into(),
                    )?
                    .unwrap_or_default())
            } else {
                panic!(
                    "Field {}.{} is of type M2O. This should not be possible here",
                    target_model.name, current_field
                )
            }
        }
    }

    /// Compare a whole relation against records, for the sides that have no cell of their own.
    ///
    /// A one2many lives in the children's foreign key and a many2many in a table of pairs, so
    /// neither can be read off the row. The PostgreSQL backend says the same thing in SQL, and
    /// both are exercised by the same tests.
    ///
    /// `None` for a many2one, which does have a cell and is already handled.
    fn _relation_rows(
        &self,
        model_name: &str,
        reference: &FieldReference,
        operator: &SearchOperator,
        right: &RightTuple,
    ) -> Result<Option<Vec<u32>>> {
        // A relation is a set, so only membership means anything against it as a whole. Refused
        // rather than answered emptily, and refused the same way the PostgreSQL backend does.
        let negated = match operator {
            SearchOperator::Equal | SearchOperator::In => false,
            SearchOperator::NotEqual | SearchOperator::NotIn => true,
            _ => {
                return Err(CacheDatabaseError::OperatorOnRelation {
                    operator: format!("{operator:?}"),
                }
                .into());
            }
        };
        // Against nothing, the question is whether any link exists at all, so an empty
        // right-hand side flips the sense.
        let wanted = match right {
            RightTuple::None => None,
            other => Some(as_ids(other)),
        };
        let negated = negated != wanted.is_none();

        let mut linked: Vec<u32> = Vec::new();
        match &reference.inverse_field {
            FieldReferenceType::M2O { .. } => return Ok(None),
            FieldReferenceType::O2M { inverse_field } => {
                let Some(table) = self.tables.get(reference.target_model) else {
                    return Ok(Some(if negated {
                        self._all_rows(model_name)
                    } else {
                        Vec::new()
                    }));
                };
                for (id, row) in &table.rows {
                    if wanted.as_ref().is_some_and(|wanted| !wanted.contains(id)) {
                        continue;
                    }
                    if let Some(FieldType::UInteger(owner)) = row.get_cell(inverse_field) {
                        linked.push(*owner);
                    }
                }
            }
            FieldReferenceType::M2M {
                relation,
                column,
                target_column,
            } => {
                let Some(table) = self.tables.get(relation) else {
                    return Ok(Some(if negated {
                        self._all_rows(model_name)
                    } else {
                        Vec::new()
                    }));
                };
                for row in table.rows.values() {
                    let (Some(FieldType::UInteger(owner)), Some(FieldType::UInteger(target))) =
                        (row.get_cell(column), row.get_cell(target_column))
                    else {
                        continue;
                    };
                    if wanted
                        .as_ref()
                        .is_some_and(|wanted| !wanted.contains(target))
                    {
                        continue;
                    }
                    linked.push(*owner);
                }
            }
        }

        let mut linked: HashSet<u32> = linked.into_iter().collect();
        if negated {
            let mut rest: Vec<u32> = self
                ._all_rows(model_name)
                .into_iter()
                .filter(|id| !linked.remove(id) && !linked.contains(id))
                .collect();
            rest.sort_unstable();
            return Ok(Some(rest));
        }
        let mut linked: Vec<u32> = linked.into_iter().collect();
        linked.sort_unstable();
        Ok(Some(linked))
    }

    /// Every record of a model, for the complement of a relation search.
    fn _all_rows(&self, model_name: &str) -> Vec<u32> {
        self.tables
            .get(model_name)
            .map(|table| table.rows.keys().copied().collect())
            .unwrap_or_default()
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
        let store = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(store.installed)
    }

    /// Initialize this database.
    ///
    /// Written straight through to the shared store: installation happens at boot, outside any
    /// transaction.
    fn initialize(&mut self) -> Result<()> {
        self.installed = true;
        let mut store = self
            .store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        store.installed = true;
        Ok(())
    }

    /// Make a search request to a specific model, and only return ids that match this search request
    fn find_ids(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
        options: &SearchOptions,
    ) -> Result<Vec<u32>> {
        let mut ids = self.get_rows(model_name, domain, model_manager)?;
        // Rows live in a `HashMap` and set operations round-trip through a `HashSet`, so the
        // natural order varies between runs. Sorting by id makes results reproducible, which is
        // what `limit` and `offset` need to mean anything, and it breaks ties for `order`.
        ids.sort_unstable();
        if !options.order.is_empty() {
            self.sort_ids(model_name, &mut ids, options);
        }
        Ok(options.paginate(ids))
    }

    fn count(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
    ) -> Result<u32> {
        Ok(self.get_rows(model_name, domain, model_manager)?.len() as u32)
    }

    /// Make a search request to a specific model, and return ids and fields that match this search request
    fn search<'a>(
        &mut self,
        model_name: &str,
        fields: &[&'a str],
        domain: &SearchType,
        model_manager: &ModelManager,
        options: &SearchOptions,
    ) -> Result<Vec<SearchedRow<'a>>> {
        // We don't care about searching 2 times (one to retrieve ids and one to retrieve fields), as it's cache
        let ids = self.find_ids(model_name, domain, model_manager, options)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }
        // Following error should never occur, as if table doesn't exist then .find_ids should return an empty list
        let table = self
            .tables
            .get(model_name)
            .unwrap_or_else(|| panic!("Table {model_name} should exist in cache"));
        let mut result = vec![];
        for id in ids.iter() {
            let mut fields_result = HashMap::new();
            // Following error should never occur, as ids returned by "find_ids" method are ids already present in table
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

    fn create(&mut self, model_name: &str, data: &[&MapOfFields]) -> Result<Vec<u32>> {
        let ids = {
            let mut store = self
                .store
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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
            table.insert_row(*id, Row { cells });
        }
        self.mark_rows(model_name, ids.iter().copied(), RowOp::Written);
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
        self.mark_rows(model_name, updated_ids, RowOp::Written);
        // If model not present in database, do nothing
        Ok(number_of_updates)
    }

    fn read_relation(
        &mut self,
        relation: &str,
        column: &str,
        target_column: &str,
        ids: &[u32],
    ) -> Result<HashMap<u32, Vec<u32>>> {
        let mut result: HashMap<u32, Vec<u32>> = ids.iter().map(|id| (*id, Vec::new())).collect();
        let Some(table) = self.tables.get(relation) else {
            return Ok(result);
        };
        for row in table.rows.values() {
            let (Some(FieldType::UInteger(owner)), Some(FieldType::UInteger(target))) =
                (row.get_cell(column), row.get_cell(target_column))
            else {
                continue;
            };
            if let Some(targets) = result.get_mut(owner) {
                targets.push(*target);
            }
        }
        for targets in result.values_mut() {
            targets.sort_unstable();
        }
        Ok(result)
    }

    fn write_relation(
        &mut self,
        relation: &str,
        column: &str,
        target_column: &str,
        id: u32,
        targets: &[u32],
    ) -> Result<()> {
        let new_ids = {
            let mut store = self
                .store
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            store.reserve_ids(relation, targets.len())
        };
        let table = self.tables.entry(relation.to_string()).or_default();

        let stale: Vec<u32> = table
            .rows
            .iter()
            .filter(|(_, row)| row.get_cell(column) == &Some(FieldType::UInteger(id)))
            .map(|(row_id, _)| *row_id)
            .collect();
        for row_id in &stale {
            table.delete_row(row_id);
        }
        for (row_id, target) in new_ids.iter().zip(targets) {
            let cells = HashMap::from([
                (column.to_string(), Some(FieldType::UInteger(id))),
                (
                    target_column.to_string(),
                    Some(FieldType::UInteger(*target)),
                ),
            ]);
            table.insert_row(*row_id, Row { cells });
        }

        self.mark_rows(relation, stale, RowOp::Deleted);
        self.mark_rows(relation, new_ids, RowOp::Written);
        Ok(())
    }

    fn delete(&mut self, model_name: &str, ids: &[u32]) -> Result<u32> {
        let mut deleted_ids = Vec::new();
        if let Some(table) = self.tables.get_mut(model_name) {
            for id in ids {
                if table.delete_row(id) {
                    deleted_ids.push(*id);
                }
            }
        }
        let number_of_deletions = deleted_ids.len() as u32;
        self.mark_rows(model_name, deleted_ids, RowOp::Deleted);
        Ok(number_of_deletions)
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
        match self.savepoints.last() {
            Some(Savepoint {
                name: Some(last), ..
            }) if last == name => {
                self.savepoints.pop();
                Ok(())
            }
            Some(_) => Err(CacheDatabaseError::NotTheLastSavepoint {
                name: name.to_string(),
            }
            .into()),
            None => Err(CacheDatabaseError::MissingSavepoint {
                name: name.to_string(),
                operation: "commit",
            }
            .into()),
        }
    }

    fn savepoint_rollback(&mut self, name: &str) -> Result<()> {
        match self.savepoints.last() {
            Some(Savepoint {
                name: Some(last), ..
            }) if last == name => {
                let savepoint = self.savepoints.pop().expect("checked just above");
                self.tables = savepoint.tables;
                self.written_rows = savepoint.written_rows;
                Ok(())
            }
            Some(_) => Err(CacheDatabaseError::NotTheLastSavepoint {
                name: name.to_string(),
            }
            .into()),
            None => Err(CacheDatabaseError::MissingSavepoint {
                name: name.to_string(),
                operation: "roll back",
            }
            .into()),
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
            written_rows: WriteSet::new(),
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
        Err(CacheDatabaseError::NoTransaction {
            operation: "commit",
        }
        .into())
    }

    fn rollback_transaction(&mut self) -> Result<()> {
        while let Some(savepoint) = self.savepoints.pop() {
            if savepoint.name.is_none() {
                self.tables = savepoint.tables;
                self.written_rows = savepoint.written_rows;
                return Ok(());
            }
        }
        Err(CacheDatabaseError::NoTransaction {
            operation: "roll back",
        }
        .into())
    }
}

/// A right-hand side as the set of ids it stands for.
fn as_ids(right: &RightTuple) -> HashSet<u32> {
    match right {
        RightTuple::Array(members) => members.iter().flat_map(as_ids).collect(),
        RightTuple::UInteger(id) => HashSet::from([*id]),
        RightTuple::Integer(id) => u32::try_from(*id).ok().into_iter().collect(),
        _ => HashSet::new(),
    }
}
