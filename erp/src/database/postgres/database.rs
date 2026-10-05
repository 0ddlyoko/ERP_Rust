use super::pool::{ConnectionPool, PooledConnection};
use super::{
    QueryBuilder, column_type, from_row, id_from_sql, id_to_sql, quote_ident, to_sql_param,
};
use crate::database::{Database, ErrorType, FieldType, SearchedRow};
use crate::model::ModelManager;
use erp_internal_types::{FinalInternalField, FinalInternalModel};
use erp_search::{SearchOptions, SearchType};
use erp_types::field::{FieldReference, FieldReferenceType};
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

    /// Columns the table already has.
    fn existing_constraints(&mut self, table_name: &str) -> Result<HashSet<String>> {
        let rows = self.client.query(
            "SELECT \"constraint_name\" FROM \"information_schema\".\"table_constraints\" \
             WHERE \"table_schema\" = $1 AND \"table_name\" = $2",
            &[&self.schema, &table_name],
        )?;
        rows.iter()
            .map(|row| Ok(row.try_get::<_, String>(0)?))
            .collect()
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
    fn sync_not_null(&mut self, model: &FinalInternalModel, qualified: &str) -> Result<()> {
        let rows = self.client.query(
            "SELECT \"column_name\", \"is_nullable\" = 'YES' FROM \"information_schema\".\"columns\" \
             WHERE \"table_schema\" = $1 AND \"table_name\" = $2",
            &[&self.schema, &model.table_name],
        )?;
        let nullable: HashMap<String, bool> = rows
            .iter()
            .map(|row| Ok((row.try_get(0)?, row.try_get(1)?)))
            .collect::<Result<_>>()?;
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
                if self.execute_or_undo(&set).is_err() {
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

    fn existing_columns(&mut self, table_name: &str) -> Result<HashSet<String>> {
        let rows = self.client.query(
            "SELECT \"column_name\" FROM \"information_schema\".\"columns\" \
             WHERE \"table_schema\" = $1 AND \"table_name\" = $2",
            &[&self.schema, &table_name],
        )?;
        rows.iter()
            .map(|row| Ok(row.try_get::<_, String>(0)?))
            .collect()
    }
}

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
        self.client.batch_execute(&format!(
            "CREATE TABLE IF NOT EXISTS {qualified} (\"id\" SERIAL PRIMARY KEY)"
        ))?;

        let existing = self.existing_columns(&model.table_name)?;
        let mut added = Vec::new();
        for (field_name, field) in &model.fields {
            if existing.contains(field_name) {
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
                    let statement = format!(
                        "CREATE TABLE IF NOT EXISTS {} ({} INTEGER NOT NULL, {} INTEGER NOT NULL, \
                         PRIMARY KEY ({}, {}))",
                        self.qualified_relation(relation),
                        quote_ident(column),
                        quote_ident(target_column),
                        quote_ident(column),
                        quote_ident(target_column)
                    );
                    self.client.batch_execute(&statement)?;
                }
                continue;
            };
            self.client.batch_execute(&format!(
                "ALTER TABLE {qualified} ADD COLUMN {} {column_type}",
                quote_ident(field_name)
            ))?;
            self.fill_default(&qualified, field)?;
            added.push(field_name.clone());
        }
        self.sync_not_null(model, &qualified)?;
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
            let existing = self.existing_constraints(relation)?;
            let target_table = self
                .tables
                .get(*target_model)
                .map_or(*target_model, String::as_str);
            let sides = [
                (column, model.table_name.as_str()),
                (target_column, target_table),
            ];
            for (side, table) in sides {
                let name = format!("{relation}_{side}_fkey");
                if existing.contains(&name) {
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
    /// One statement per record: the maps may not share the same set of fields, and grouping
    /// them would trade clarity for a round trip that the flush already batches elsewhere.
    fn create(&mut self, model_name: &str, data: &[&MapOfFields]) -> Result<Vec<u32>> {
        let table = self.qualified_table(model_name)?;
        let mut ids = Vec::with_capacity(data.len());
        for record in data {
            let mut builder = QueryBuilder::new();
            let mut columns = Vec::new();
            let mut placeholders = Vec::new();
            for (field_name, value) in &record.fields {
                if field_name == "id" {
                    continue;
                }
                let Some(value) = value else {
                    continue;
                };
                columns.push(quote_ident(field_name));
                placeholders.push(builder.push_value(&value.clone().into())?);
            }
            let sql = if columns.is_empty() {
                format!(
                    "INSERT INTO {table} DEFAULT VALUES RETURNING {}",
                    quote_ident("id")
                )
            } else {
                format!(
                    "INSERT INTO {table} ({}) VALUES ({}) RETURNING {}",
                    columns.join(", "),
                    placeholders.join(", "),
                    quote_ident("id")
                )
            };
            let row = self.client.query_one(&sql, &builder.params())?;
            ids.push(id_from_sql(row.try_get::<_, i32>(0)?)?);
        }
        Ok(ids)
    }

    /// Update records, returning how many rows were actually touched.
    ///
    /// An id that is not there is skipped rather than reported, mirroring the in-memory backend.
    fn update(&mut self, model_name: &str, data: &HashMap<u32, &MapOfFields>) -> Result<u32> {
        let table = self.qualified_table(model_name)?;
        let mut updated = 0;
        for (id, record) in data {
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
            let id_placeholder = builder.push_value(&FieldType::UInteger(*id))?;
            let sql = format!(
                "UPDATE {table} SET {} WHERE {} = {id_placeholder}",
                assignments.join(", "),
                quote_ident("id")
            );
            updated += self.client.execute(&sql, &builder.params())? as u32;
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
