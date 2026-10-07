use super::pool::{ConnectionPool, PooledConnection};
use super::{
    QueryBuilder, column_type, from_row, id_from_sql, id_to_sql, quote_ident, to_sql_param,
};
use crate::database::{Database, ErrorType, FieldType, Group, GroupBy, SearchedRow};
use crate::model::ModelManager;
use erp_internal_types::{FinalInternalField, FinalInternalModel};
use erp_search::{SearchOptions, SearchType};
use erp_types::field::{FieldIndex, FieldKind, FieldReference, FieldReferenceType};
use erp_types::model::MapOfFields;
use std::collections::{HashMap, HashSet};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub struct PostgresDatabase {
    pub client: PooledConnection,
    schema: String,
    is_transaction: bool,
    /// Model identity to physical table.
    ///
    /// Seeded from the registry when the connection is opened, because a model stored under
    /// another name has to be reachable from every connection, not only the one that created
    /// its table.
    tables: HashMap<String, String>,
    /// The schema as it stands, read once while the plugins load and kept in step with what is
    /// created; `None` until first needed.
    pub(crate) schema_state: Option<SchemaState>,
}

/// The tables of the schema with their columns — and whether each may be empty — their
/// constraints, and the indexes there are: what bringing the schema in line with the models
/// compares with, read in three queries rather than a few per model.
#[derive(Default)]
pub struct SchemaState {
    columns: HashMap<String, HashMap<String, bool>>,
    constraints: HashMap<String, HashSet<String>>,
    indexes: HashSet<String>,
}

impl PostgresDatabase {
    /// Take a connection from the pool.
    ///
    /// Held for as long as the environment that asked for it, so the pool's size is the ceiling
    /// on transactions running at once. Returned by dropping, including on rollback.
    pub(crate) fn connect(
        pool: &ConnectionPool,
        schema: &str,
        tables: HashMap<String, String>,
    ) -> std::result::Result<Self, ErrorType> {
        Ok(Self {
            client: pool.get()?,
            schema: schema.to_string(),
            is_transaction: false,
            tables,
            schema_state: None,
        })
    }
}

impl PostgresDatabase {
    /// Table backing a model, quoted and schema-qualified.
    ///
    /// A model the registry never mentioned falls back to its own name, which is also the
    /// default table name.
    fn qualified_table(&self, model_name: &str) -> Result<String> {
        let table = self
            .tables
            .get(model_name)
            .map_or(model_name, String::as_str);
        Ok(format!(
            "{}.{}",
            quote_ident(&self.schema),
            quote_ident(table)
        ))
    }

    /// A relation table, quoted and schema-qualified.
    fn qualified_relation(&self, relation: &str) -> String {
        format!("{}.{}", quote_ident(&self.schema), quote_ident(relation))
    }

    /// The schema as it stands, read the first time it is needed.
    fn state(&mut self) -> Result<&mut SchemaState> {
        if self.schema_state.is_none() {
            let mut state = SchemaState::default();
            for row in self.client.query(
                "SELECT \"relname\"::text, \"attname\"::text, NOT \"attnotnull\" FROM \"pg_attribute\" \
                 JOIN \"pg_class\" ON \"pg_class\".\"oid\" = \"attrelid\" \
                 JOIN \"pg_namespace\" ON \"pg_namespace\".\"oid\" = \"relnamespace\" \
                 WHERE \"nspname\" = $1 AND \"relkind\" = 'r' AND \"attnum\" > 0 AND NOT \"attisdropped\"",
                &[&self.schema],
            )? {
                state
                    .columns
                    .entry(row.try_get(0)?)
                    .or_default()
                    .insert(row.try_get(1)?, row.try_get(2)?);
            }
            for row in self.client.query(
                "SELECT \"relname\"::text, \"conname\"::text FROM \"pg_constraint\" \
                 JOIN \"pg_class\" ON \"pg_class\".\"oid\" = \"conrelid\" \
                 JOIN \"pg_namespace\" ON \"pg_namespace\".\"oid\" = \"relnamespace\" \
                 WHERE \"nspname\" = $1",
                &[&self.schema],
            )? {
                state
                    .constraints
                    .entry(row.try_get(0)?)
                    .or_default()
                    .insert(row.try_get(1)?);
            }
            for row in self.client.query(
                "SELECT \"relname\"::text FROM \"pg_class\" \
                 JOIN \"pg_namespace\" ON \"pg_namespace\".\"oid\" = \"relnamespace\" \
                 WHERE \"nspname\" = $1 AND \"relkind\" = 'i'",
                &[&self.schema],
            )? {
                state.indexes.insert(row.try_get(0)?);
            }
            self.schema_state = Some(state);
        }
        Ok(self.schema_state.as_mut().expect("read above"))
    }

    /// Create a table unless it is there, with the columns given; as the schema state says.
    fn create_table(
        &mut self,
        table: &str,
        definition: &str,
        columns: &[(&str, bool)],
    ) -> Result<()> {
        if self.state()?.columns.contains_key(table) {
            return Ok(());
        }
        self.client.batch_execute(&format!(
            "CREATE TABLE IF NOT EXISTS {} ({definition})",
            self.qualified_relation(table)
        ))?;
        let columns = columns
            .iter()
            .map(|(name, nullable)| (name.to_string(), *nullable))
            .collect();
        self.state()?.columns.insert(table.to_string(), columns);
        Ok(())
    }

    /// Give the rows already there a new column's default, as a record created now would get.
    ///
    /// A computed column is left to its computation, which fills it once the schema is in place.
    fn fill_default(&mut self, qualified: &str, field: &FinalInternalField) -> Result<()> {
        let Some(default) = &field.default_value else {
            return Ok(());
        };
        if field.compute.is_some() {
            return Ok(());
        }
        let value = to_sql_param(&FieldType::from(default.clone()))?;
        self.client.execute(
            &format!("UPDATE {qualified} SET {} = $1", quote_ident(&field.name)),
            &[&*value],
        )?;
        Ok(())
    }

    /// Hold every required column to `NOT NULL`, and free those no longer required.
    ///
    /// Computed and automatic columns are left free: a computed one is filled after its row is
    /// inserted. When rows already hold no value, the constraint cannot be added: the server says
    /// so and starts anyway, the ORM still refusing empty values.
    fn sync_not_null(
        &mut self,
        model: &FinalInternalModel,
        qualified: &str,
        nullable: &mut HashMap<String, bool>,
    ) -> Result<()> {
        let mut fields: Vec<&FinalInternalField> = model.fields.values().collect();
        fields.sort_by_key(|field| &field.name);
        for field in fields {
            let Some(&is_nullable) = nullable.get(&field.name) else {
                continue;
            };
            let column = quote_ident(&field.name);
            let wants_not_null = field.required && field.compute.is_none() && !field.automatic;
            if wants_not_null && is_nullable {
                let set = format!("ALTER TABLE {qualified} ALTER COLUMN {column} SET NOT NULL");
                if self.execute_or_undo(&set).is_ok() {
                    nullable.insert(field.name.clone(), false);
                } else {
                    let empty: i64 = self
                        .client
                        .query_one(
                            &format!("SELECT COUNT(*) FROM {qualified} WHERE {column} IS NULL"),
                            &[],
                        )?
                        .try_get(0)?;
                    tracing::warn!(
                        "{}.{}: cannot be NOT NULL, {empty} rows hold no value",
                        model.name,
                        field.name
                    );
                }
            } else if !wants_not_null && !is_nullable {
                self.client.batch_execute(&format!(
                    "ALTER TABLE {qualified} ALTER COLUMN {column} DROP NOT NULL"
                ))?;
                nullable.insert(field.name.clone(), true);
            }
        }
        Ok(())
    }

    /// Run a statement that may fail, without its failure ending the transaction around it.
    fn execute_or_undo(&mut self, statement: &str) -> Result<()> {
        if !self.is_transaction {
            return Ok(self.client.batch_execute(statement)?);
        }
        self.client.batch_execute("SAVEPOINT \"may_fail\"")?;
        match self.client.batch_execute(statement) {
            Ok(()) => Ok(self.client.batch_execute("RELEASE \"may_fail\"")?),
            Err(error) => {
                self.client.batch_execute("ROLLBACK TO \"may_fail\"")?;
                Err(error.into())
            }
        }
    }

    /// Index `column` of `table`.
    fn create_index(&mut self, table: &str, column: &str, index: FieldIndex) -> Result<()> {
        let name = index_name(table, column);
        let qualified = self.qualified_relation(table);
        let plain = format!(
            "CREATE INDEX IF NOT EXISTS {} ON {qualified} ({})",
            quote_ident(&name),
            quote_ident(column)
        );
        if index == FieldIndex::Btree {
            return Ok(self.client.batch_execute(&plain)?);
        }
        if let Err(error) = self.create_trigram_index(&name, &qualified, column) {
            tracing::warn!(%error, table, column, "No trigram index: a plain one instead");
            self.client.batch_execute(&plain)?;
        }
        Ok(())
    }

    /// The trigram index, with `pg_trgm` installed first if it is not — in `public`, so that no
    /// schema of ours holds it and dropping one cannot take it, and every trigram index, away.
    /// Its operator class is named in the schema it lives in, which `search_path` leaves out.
    fn create_trigram_index(&mut self, name: &str, qualified: &str, column: &str) -> Result<()> {
        self.execute_or_undo("CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public")?;
        let row = self.client.query_one(
            "SELECT \"nspname\" FROM \"pg_extension\" JOIN \"pg_namespace\" \
             ON \"pg_namespace\".\"oid\" = \"extnamespace\" WHERE \"extname\" = 'pg_trgm'",
            &[],
        )?;
        let extension_schema: String = row.try_get(0)?;
        self.execute_or_undo(&format!(
            "CREATE INDEX IF NOT EXISTS {} ON {qualified} USING gin ({} {}.gin_trgm_ops)",
            quote_ident(name),
            quote_ident(column),
            quote_ident(&extension_schema)
        ))
    }
}

/// The name of the index on `column` of `table`.
fn index_name(table: &str, column: &str) -> String {
    constraint_name(&format!("{table}_{column}_index"))
}

/// How many parameters one statement may bind: PostgreSQL counts them on 16 bits.
const MAX_PARAMETERS: usize = 65_000;

impl Database for PostgresDatabase {
    /// Check if given database is already installed
    fn is_installed(&mut self) -> Result<bool> {
        let result = self.client.query_one(
            "SELECT EXISTS (
            SELECT FROM \"pg_tables\" WHERE \"schemaname\"=$1 AND \"tablename\"='plugin'
        )",
            &[&self.schema],
        )?;
        Ok(result.try_get(0)?)
    }

    /// Initialize this database.
    ///
    /// Only the bootstrap table is created here; every other table comes from the models
    /// themselves, through [`Database::sync_model`].
    fn initialize(&mut self) -> Result<()> {
        let schema = quote_ident(&self.schema);
        self.client.batch_execute(&format!(
            "CREATE SCHEMA IF NOT EXISTS {schema};
             CREATE TABLE IF NOT EXISTS {schema}.\"plugin\" (
                \"id\"          SERIAL PRIMARY KEY,
                \"name\"        TEXT NOT NULL,
                \"description\" TEXT,
                \"website\"     TEXT,
                \"url\"         TEXT,
                \"state\"       TEXT NOT NULL
             )"
        ))?;
        Ok(())
    }

    /// Create the model's table, or add the columns it has gained.
    ///
    /// Driven by introspection rather than by migration files: nothing records a schema version
    /// yet, so the current shape of the database is the only reference available.
    fn sync_model(
        &mut self,
        model: &erp_internal_types::FinalInternalModel,
    ) -> Result<Vec<String>> {
        self.tables
            .insert(model.name.clone(), model.table_name.clone());
        let qualified = format!(
            "{}.{}",
            quote_ident(&self.schema),
            quote_ident(&model.table_name)
        );

        // `id` is never in the registry, so it is synthesised here.
        self.create_table(
            &model.table_name,
            "\"id\" SERIAL PRIMARY KEY",
            &[("id", false)],
        )?;

        let mut existing = self.state()?.columns[&model.table_name].clone();
        let mut added = Vec::new();
        for (field_name, field) in &model.fields {
            if existing.contains_key(field_name) {
                continue;
            }
            // A computed field that is worked out on each read has nowhere to be: no column, and
            // no table of pairs if it is a relation.
            if !field.stored {
                continue;
            }
            let Some(column_type) = column_type(field.kind) else {
                // A one2many has no column: it is read from the other side's foreign key. A
                // many2many has none either, but it does have a table of pairs.
                if let Some(FieldReference {
                    inverse_field:
                        FieldReferenceType::M2M {
                            relation,
                            column,
                            target_column,
                        },
                    ..
                }) = &field.inverse
                {
                    let definition = format!(
                        "{} INTEGER NOT NULL, {} INTEGER NOT NULL, PRIMARY KEY ({}, {})",
                        quote_ident(column),
                        quote_ident(target_column),
                        quote_ident(column),
                        quote_ident(target_column)
                    );
                    self.create_table(
                        relation,
                        &definition,
                        &[(column.as_str(), false), (target_column.as_str(), false)],
                    )?;
                }
                continue;
            };
            self.client.batch_execute(&format!(
                "ALTER TABLE {qualified} ADD COLUMN {} {column_type}",
                quote_ident(field_name)
            ))?;
            self.fill_default(&qualified, field)?;
            added.push(field_name.clone());
            existing.insert(field_name.clone(), true);
        }
        self.sync_not_null(model, &qualified, &mut existing)?;
        if let Some(columns) = self.state()?.columns.get_mut(&model.table_name) {
            columns.extend(existing);
        }
        Ok(added)
    }

    /// Tie each relation table to the two tables it pairs, so a deleted record cannot leave a
    /// pair behind.
    ///
    /// The relation table carries no data of its own — a pair that outlives one of its ends is
    /// never anything but wrong, and would resurface the day the database reuses the id. Letting
    /// the server enforce it also means a row removed by hand, or by a migration, stays
    /// consistent.
    fn sync_constraints(&mut self, model: &erp_internal_types::FinalInternalModel) -> Result<()> {
        for field in model.fields.values() {
            let Some(FieldReference {
                target_model,
                inverse_field:
                    FieldReferenceType::M2M {
                        relation,
                        column,
                        target_column,
                    },
            }) = &field.inverse
            else {
                continue;
            };
            let existing = self
                .state()?
                .constraints
                .get(relation.as_str())
                .cloned()
                .unwrap_or_default();
            let target_table = self
                .tables
                .get(*target_model)
                .map_or(*target_model, String::as_str)
                .to_string();
            let sides = [
                (column, model.table_name.as_str()),
                (target_column, target_table.as_str()),
            ];
            for (side, table) in sides {
                let full = format!("{relation}_{side}_fkey");
                let name = constraint_name(&full);
                if existing.contains(&name) || existing.contains(truncated(&full)) {
                    continue;
                }
                let statement = format!(
                    "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}.{} ({}) \
                     ON DELETE CASCADE",
                    self.qualified_relation(relation),
                    quote_ident(&name),
                    quote_ident(side),
                    quote_ident(&self.schema),
                    quote_ident(table),
                    quote_ident("id"),
                );
                self.client.batch_execute(&statement)?;
                self.state()?
                    .constraints
                    .entry(relation.clone())
                    .or_default()
                    .insert(name);
            }
        }
        Ok(())
    }

    /// Index the columns asked for, every many2one but the automatic ones, and the far side of
    /// each many2many — its near side leads the table's primary key already.
    ///
    /// A trigram index needs `pg_trgm`; without the right to install it, a plain one is made and
    /// the search stays correct, only slower.
    fn sync_indexes(&mut self, model: &erp_internal_types::FinalInternalModel) -> Result<()> {
        let mut wanted: Vec<(String, String, FieldIndex)> = Vec::new();
        for (field_name, field) in &model.fields {
            if !field.stored {
                continue;
            }
            if let Some(FieldReference {
                inverse_field:
                    FieldReferenceType::M2M {
                        relation,
                        target_column,
                        ..
                    },
                ..
            }) = &field.inverse
            {
                wanted.push((relation.clone(), target_column.clone(), FieldIndex::Btree));
                continue;
            }
            let index = match field.index {
                Some(index) => index,
                None if field.kind == FieldKind::Ref && !field.automatic => FieldIndex::Btree,
                None => continue,
            };
            let index = match field.kind {
                FieldKind::String => index,
                _ => FieldIndex::Btree,
            };
            wanted.push((model.table_name.clone(), field_name.clone(), index));
        }
        for (table, column, index) in wanted {
            let name = index_name(&table, &column);
            if !self.state()?.indexes.contains(&name) {
                self.create_index(&table, &column, index)?;
                self.state()?.indexes.insert(name);
            }
        }
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
        let mut builder = QueryBuilder::new();
        let sql = builder.select_ids(model_name, domain, model_manager, options)?;
        let rows = self.client.query(&sql, &builder.params())?;
        rows.iter()
            .map(|row| id_from_sql(row.try_get::<_, i32>(0)?))
            .collect()
    }

    fn count(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
    ) -> Result<u32> {
        let mut builder = QueryBuilder::new();
        let sql = builder.select_count(model_name, domain, model_manager)?;
        let row = self.client.query_one(&sql, &builder.params())?;
        Ok(row.try_get::<_, i64>(0)? as u32)
    }

    /// Gathered by the database itself: a date by period is truncated to the first day of it.
    fn read_group(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        group_by: Option<&GroupBy>,
        sums: &[&str],
        model_manager: &ModelManager,
    ) -> Result<Vec<Group>> {
        let model = model_manager.try_get_model(model_name)?;
        let key = group_by.map(|group_by| match group_by.period {
            Some(period) => format!(
                "(date_trunc('{}', {}))::date",
                period.key(),
                quote_ident(&group_by.field)
            ),
            None => quote_ident(&group_by.field),
        });
        let mut builder = QueryBuilder::new();
        let sql = builder.select_group(model_name, key.as_deref(), sums, domain, model_manager)?;
        let rows = self.client.query(&sql, &builder.params())?;
        let first = usize::from(group_by.is_some());
        let mut groups = Vec::with_capacity(rows.len());
        for row in rows {
            let key = match group_by {
                None => None,
                Some(GroupBy {
                    period: Some(_), ..
                }) => row
                    .try_get::<_, Option<chrono::NaiveDate>>(0)?
                    .map(FieldType::Date),
                Some(GroupBy { field, .. }) => {
                    from_row(&row, 0, model.try_get_internal_field(field)?.kind)?
                }
            };
            let count = row.try_get::<_, i64>(first)? as u32;
            let mut totals = HashMap::with_capacity(sums.len());
            for (index, sum) in sums.iter().enumerate() {
                let at = first + 1 + index;
                let total = match model.try_get_internal_field(sum)?.kind {
                    FieldKind::Integer => row
                        .try_get::<_, Option<i64>>(at)?
                        .map(rust_decimal::Decimal::from),
                    _ => row.try_get::<_, Option<rust_decimal::Decimal>>(at)?,
                };
                totals.insert(sum.to_string(), total.unwrap_or_default());
            }
            groups.push(Group {
                key,
                count,
                sums: totals,
            });
        }
        Ok(groups)
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
        let model = model_manager.try_get_model(model_name)?;
        let mut builder = QueryBuilder::new();
        let sql = builder.select_columns(model_name, fields, domain, model_manager, options)?;
        let rows = self.client.query(&sql, &builder.params())?;

        // The statement selects the id first, then the stored fields in the order asked for.
        let stored: Vec<&'a str> = fields
            .iter()
            .filter(|field| **field != "id")
            .filter(|field| {
                model
                    .try_get_internal_field(field)
                    .is_ok_and(erp_internal_types::FinalInternalField::is_stored)
            })
            .copied()
            .collect();

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let id = id_from_sql(row.try_get::<_, i32>(0)?)?;
            let mut values = HashMap::with_capacity(fields.len());
            for field in fields {
                if *field == "id" {
                    values.insert(*field, Some(FieldType::UInteger(id)));
                }
            }
            for (index, field) in stored.iter().enumerate() {
                let kind = model.try_get_internal_field(field)?.kind;
                values.insert(*field, from_row(&row, index + 1, kind)?);
            }
            // Fields with no column of their own simply come back empty.
            for field in fields {
                values.entry(*field).or_insert(None);
            }
            result.push((id, values));
        }
        Ok(result)
    }

    /// Insert records and return their ids, in the order the data was given.
    ///
    /// Records setting the same columns go in one statement, as many rows as its parameters
    /// allow. The ids its rows get come back sorted to match them: a serial column takes its
    /// values one row after another, in the order the rows are written.
    fn create(&mut self, model_name: &str, data: &[&MapOfFields]) -> Result<Vec<u32>> {
        let table = self.qualified_table(model_name)?;
        let mut ids = vec![0; data.len()];
        let mut groups: Vec<(Vec<&str>, Vec<usize>)> = Vec::new();
        for (row, record) in data.iter().enumerate() {
            let mut columns: Vec<&str> = record
                .fields
                .iter()
                .filter(|(name, value)| *name != "id" && value.is_some())
                .map(|(name, _)| name.as_str())
                .collect();
            columns.sort_unstable();
            match groups.iter_mut().find(|(known, _)| *known == columns) {
                Some((_, rows)) => rows.push(row),
                None => groups.push((columns, vec![row])),
            }
        }
        for (columns, rows) in groups {
            if columns.is_empty() {
                let sql = format!(
                    "INSERT INTO {table} DEFAULT VALUES RETURNING {}",
                    quote_ident("id")
                );
                for row in rows {
                    let inserted = self.client.query_one(&sql, &[])?;
                    ids[row] = id_from_sql(inserted.try_get::<_, i32>(0)?)?;
                }
                continue;
            }
            let quoted: Vec<String> = columns.iter().map(|column| quote_ident(column)).collect();
            for chunk in rows.chunks((MAX_PARAMETERS / columns.len()).max(1)) {
                let mut builder = QueryBuilder::new();
                let mut tuples = Vec::with_capacity(chunk.len());
                for &row in chunk {
                    let mut placeholders = Vec::with_capacity(columns.len());
                    for column in &columns {
                        if let Some(value) = &data[row].fields[*column] {
                            placeholders.push(builder.push_value(&value.clone().into())?);
                        }
                    }
                    tuples.push(format!("({})", placeholders.join(", ")));
                }
                let sql = format!(
                    "INSERT INTO {table} ({}) VALUES {} RETURNING {}",
                    quoted.join(", "),
                    tuples.join(", "),
                    quote_ident("id")
                );
                let mut inserted = self
                    .client
                    .query(&sql, &builder.params())?
                    .iter()
                    .map(|row| id_from_sql(row.try_get::<_, i32>(0)?))
                    .collect::<Result<Vec<u32>>>()?;
                inserted.sort_unstable();
                for (&row, id) in chunk.iter().zip(inserted) {
                    ids[row] = id;
                }
            }
        }
        Ok(ids)
    }

    /// Update records, returning how many rows were actually touched.
    ///
    /// Records given the same values are updated by one statement. An id that is not there is
    /// skipped rather than reported, mirroring the in-memory backend.
    fn update(&mut self, model_name: &str, data: &HashMap<u32, &MapOfFields>) -> Result<u32> {
        let table = self.qualified_table(model_name)?;
        let mut groups: Vec<(&MapOfFields, Vec<u32>)> = Vec::new();
        let mut sorted: Vec<(&u32, &&MapOfFields)> = data.iter().collect();
        sorted.sort_unstable_by_key(|(id, _)| **id);
        for (id, record) in sorted {
            match groups.iter_mut().find(|(values, _)| *values == *record) {
                Some((_, ids)) => ids.push(*id),
                None => groups.push((record, vec![*id])),
            }
        }
        let mut updated = 0;
        for (record, ids) in groups {
            let mut builder = QueryBuilder::new();
            let mut assignments = Vec::new();
            for (field_name, value) in &record.fields {
                if field_name == "id" {
                    continue;
                }
                let placeholder = match value {
                    Some(value) => builder.push_value(&value.clone().into())?,
                    None => "NULL".to_string(),
                };
                assignments.push(format!("{} = {placeholder}", quote_ident(field_name)));
            }
            if assignments.is_empty() {
                continue;
            }
            let ids: Vec<i32> = ids.into_iter().map(id_to_sql).collect::<Result<_>>()?;
            let sql = format!(
                "UPDATE {table} SET {} WHERE {} = ANY(${})",
                assignments.join(", "),
                quote_ident("id"),
                builder.params().len() + 1
            );
            let mut params = builder.params();
            params.push(&ids);
            updated += self.client.execute(&sql, &params)? as u32;
        }
        Ok(updated)
    }

    fn read_relation(
        &mut self,
        relation: &str,
        column: &str,
        target_column: &str,
        ids: &[u32],
    ) -> Result<HashMap<u32, Vec<u32>>> {
        let mut result: HashMap<u32, Vec<u32>> = ids.iter().map(|id| (*id, Vec::new())).collect();
        if ids.is_empty() {
            return Ok(result);
        }
        let owners: Vec<i32> = ids.iter().copied().map(id_to_sql).collect::<Result<_>>()?;
        let sql = format!(
            "SELECT {}, {} FROM {} WHERE {} = ANY($1) ORDER BY {}",
            quote_ident(column),
            quote_ident(target_column),
            self.qualified_relation(relation),
            quote_ident(column),
            quote_ident(target_column)
        );
        for row in self.client.query(&sql, &[&owners])? {
            let owner = id_from_sql(row.try_get::<_, i32>(0)?)?;
            let target = id_from_sql(row.try_get::<_, i32>(1)?)?;
            if let Some(targets) = result.get_mut(&owner) {
                targets.push(target);
            }
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
        let table = self.qualified_relation(relation);
        let owner = id_to_sql(id)?;
        self.client.execute(
            &format!("DELETE FROM {table} WHERE {} = $1", quote_ident(column)),
            &[&owner],
        )?;
        if targets.is_empty() {
            return Ok(());
        }
        let targets: Vec<i32> = targets
            .iter()
            .copied()
            .map(id_to_sql)
            .collect::<Result<_>>()?;
        self.client.execute(
            &format!(
                "INSERT INTO {table} ({}, {}) SELECT $1, * FROM UNNEST($2::INTEGER[])",
                quote_ident(column),
                quote_ident(target_column)
            ),
            &[&owner, &targets],
        )?;
        Ok(())
    }

    fn delete(&mut self, model_name: &str, ids: &[u32]) -> Result<u32> {
        if ids.is_empty() {
            return Ok(0);
        }
        let table = self.qualified_table(model_name)?;
        let ids: Vec<i32> = ids.iter().copied().map(id_to_sql).collect::<Result<_>>()?;
        let sql = format!("DELETE FROM {table} WHERE {} = ANY($1)", quote_ident("id"));
        Ok(self.client.execute(&sql, &[&ids])? as u32)
    }

    /// `SELECT … FOR UPDATE`, in the order of the ids, so two transactions locking the same rows
    /// take them in the same order rather than each waiting for the other.
    fn lock(&mut self, model_name: &str, ids: &[u32]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let table = self.qualified_table(model_name)?;
        let ids: Vec<i32> = ids.iter().copied().map(id_to_sql).collect::<Result<_>>()?;
        let id = quote_ident("id");
        let sql = format!("SELECT {id} FROM {table} WHERE {id} = ANY($1) ORDER BY {id} FOR UPDATE");
        self.client.query(&sql, &[&ids])?;
        Ok(())
    }

    fn get_installed_plugins(&mut self) -> Result<Vec<String>> {
        let mut result = vec![];
        for row in self.client.query(
            &format!(
                "SELECT \"name\" FROM {}.\"plugin\" WHERE \"state\" = 'installed'",
                quote_ident(&self.schema)
            ),
            &[],
        )? {
            let name: &str = row.get(0);
            result.push(name.to_string());
        }
        Ok(result)
    }

    fn savepoint(&mut self, name: &str) -> Result<()> {
        // Quoted like any other identifier. Names are generated internally today, so this
        // guards a door nobody can reach — which is the point of guarding it.
        Ok(self
            .client
            .batch_execute(&format!("SAVEPOINT {}", quote_ident(name)))?)
    }

    fn savepoint_commit(&mut self, name: &str) -> Result<()> {
        Ok(self
            .client
            .batch_execute(&format!("RELEASE {}", quote_ident(name)))?)
    }

    fn savepoint_rollback(&mut self, name: &str) -> Result<()> {
        Ok(self
            .client
            .batch_execute(&format!("ROLLBACK TO {}", quote_ident(name)))?)
    }

    fn start_transaction(&mut self) -> Result<()> {
        self.is_transaction = true;
        self.client.begin();
        Ok(())
    }

    fn commit_transaction(&mut self) -> Result<()> {
        self.is_transaction = false;
        Ok(self.client.commit()?)
    }

    /// Roll the transaction back. A connection that could not roll back is closed rather than
    /// lent again: what state it was left in is unknown.
    fn rollback_transaction(&mut self) -> Result<()> {
        self.is_transaction = false;
        let rolled_back = self.client.rollback();
        if rolled_back.is_err() {
            self.client.mark_broken();
        }
        Ok(rolled_back?)
    }
}

impl Drop for PostgresDatabase {
    fn drop(&mut self) {
        // Rollback if needed
        if self.is_transaction {
            let _ = self.rollback_transaction();
        }
    }
}

/// The longest name PostgreSQL keeps whole: it cuts longer ones at this many bytes.
const MAX_IDENTIFIER: usize = 63;

/// `full` when PostgreSQL keeps it whole, else its start and a hash of all of it, so that two
/// long names starting alike stay apart.
fn constraint_name(full: &str) -> String {
    if full.len() <= MAX_IDENTIFIER {
        return full.to_string();
    }
    let hash = full.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{}_{:016x}", truncated_to(full, MAX_IDENTIFIER - 17), hash)
}

/// `full` as PostgreSQL stores it: what an older version left in the database under that name.
fn truncated(full: &str) -> &str {
    truncated_to(full, MAX_IDENTIFIER)
}

fn truncated_to(full: &str, bytes: usize) -> &str {
    let mut end = bytes.min(full.len());
    while !full.is_char_boundary(end) {
        end -= 1;
    }
    &full[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_names_fit() {
        assert_eq!(constraint_name("pair_left_id_fkey"), "pair_left_id_fkey");
        let long = "account_bank_statement_line_match_rel_account_bank_statement_line_id_fkey";
        let other = "account_bank_statement_line_match_rel_account_bank_statement_line_other_fkey";
        assert_eq!(constraint_name(long).len(), MAX_IDENTIFIER);
        assert_eq!(
            constraint_name(long),
            constraint_name(long),
            "the same every time"
        );
        assert_ne!(constraint_name(long), constraint_name(other));
        assert_eq!(truncated(long).len(), MAX_IDENTIFIER);
    }
}
