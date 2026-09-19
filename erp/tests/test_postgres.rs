//! Integration tests against a real PostgreSQL server.
//!
//! They are skipped, not failed, when no server is reachable, so `cargo test` stays green on a
//! machine without one. Each test owns its schema and drops it on the way in and out, so runs do
//! not interfere.
use erp::app::Application;
use erp::config::Config;
use erp::database::{Database, DatabaseConfig, DatabaseType};
use erp_search::{OrderBy, SearchOptions};
use erp_search_code_gen::make_domain;
use erp_types::field::{Decimal, IdMode, MultipleIds, NaiveDate, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use test_utilities::models::{Invoice, SaleOrder, SaleOrderLine, Tag};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

const DATABASE: &str = "erp_rust_test";

fn config_for(schema: &str) -> Config {
    Config {
        database: DatabaseConfig {
            // A path means a unix socket, which is how a local server authenticates by peer.
            url: std::env::var("ERP_TEST_PGHOST").unwrap_or("/var/run/postgresql".to_string()),
            port: 5432,
            name: std::env::var("ERP_TEST_PGDATABASE").unwrap_or(DATABASE.to_string()),
            schema: schema.to_string(),
            user: std::env::var("PGUSER")
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_default(),
            password: std::env::var("PGPASSWORD").unwrap_or_default(),
        },
        plugin_path: String::new(),
    }
}

/// An application backed by PostgreSQL, with its models synchronised into a fresh schema.
///
/// Returns `None` when no server answers, which the callers report as a skip.
fn postgres_app(schema: &str) -> Option<Application> {
    let mut app = Application::new(config_for(schema));
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.post_register();

    let mut database = match app.create_new_database() {
        Ok(database) => database,
        Err(_) => return None,
    };
    let DatabaseType::Postgres(connection) = &mut database else {
        return None;
    };
    // Start from nothing, so a previous run cannot colour the result.
    connection
        .client
        .batch_execute(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
        .ok()?;
    database.initialize().ok()?;
    for name in ["invoice", "sale_order", "sale_order_line", "tag"] {
        let model = app.model_manager.get_model(name);
        database.sync_model(model).ok()?;
    }
    drop(database);
    Some(app)
}

macro_rules! app_or_skip {
    ($schema:expr) => {
        match postgres_app($schema) {
            Some(app) => app,
            None => {
                eprintln!("skipping: no PostgreSQL server reachable");
                return Ok(());
            }
        }
    };
}

/// The schema is built from the model declarations, not from a migration file.
#[test]
fn test_schema_is_generated_from_the_models() -> Result<()> {
    let app = app_or_skip!("t_schema");
    let mut database = app.create_new_database()?;
    let DatabaseType::Postgres(connection) = &mut database else {
        unreachable!()
    };

    let columns: Vec<(String, String)> = connection
        .client
        .query(
            "SELECT column_name, data_type FROM information_schema.columns \
             WHERE table_schema = 't_schema' AND table_name = 'invoice' ORDER BY column_name",
            &[],
        )?
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();

    let by_name: HashMap<&str, &str> = columns
        .iter()
        .map(|(name, kind)| (name.as_str(), kind.as_str()))
        .collect();
    assert_eq!(by_name.get("id"), Some(&"integer"), "id is synthesised");
    assert_eq!(by_name.get("name"), Some(&"text"));
    assert_eq!(by_name.get("amount_untaxed"), Some(&"numeric"));
    assert_eq!(by_name.get("due_date"), Some(&"date"));
    assert_eq!(by_name.get("created_at"), Some(&"timestamp with time zone"));
    Ok(())
}

/// A one2many has no column of its own.
#[test]
fn test_one2many_has_no_column() -> Result<()> {
    let app = app_or_skip!("t_o2m");
    let mut database = app.create_new_database()?;
    let DatabaseType::Postgres(connection) = &mut database else {
        unreachable!()
    };

    let columns: Vec<String> = connection
        .client
        .query(
            "SELECT column_name FROM information_schema.columns \
             WHERE table_schema = 't_o2m' AND table_name = 'sale_order'",
            &[],
        )?
        .iter()
        .map(|row| row.get(0))
        .collect();

    assert!(columns.contains(&"total_price".to_string()));
    assert!(
        !columns.contains(&"lines".to_string()),
        "a one2many is read from the other side's foreign key"
    );
    Ok(())
}

/// Records survive a commit and a fresh connection — the whole point of the lot.
#[test]
fn test_records_persist_across_connections() -> Result<()> {
    let app = app_or_skip!("t_persist");

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "INV-100");
    map.insert("amount_untaxed", Decimal::from_str("1234.56")?);
    map.insert("due_date", NaiveDate::from_str("2026-07-01")?);
    let _invoice: Invoice<SingleId> = env.create_new_record_from_map(map)?;
    env.close()?;

    let mut env = app.new_env()?;
    let found: Invoice<MultipleIds> = env.search(&make_domain!([("name", "=", "INV-100")]))?;
    assert_eq!(found.id.get_ids_ref().len(), 1);
    assert_eq!(
        found.get_amount_untaxed(&mut env)?,
        vec![&Decimal::from_str("1234.56")?],
        "a decimal must come back exact"
    );
    Ok(())
}

/// A rolled back transaction must leave nothing behind.
#[test]
fn test_rollback_leaves_nothing() -> Result<()> {
    let app = app_or_skip!("t_rollback");

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "ghost");
    let _invoice: Invoice<SingleId> = env.create_new_record_from_map(map)?;
    drop(env);

    let mut env = app.new_env()?;
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "=", "ghost")]))?,
        0
    );
    Ok(())
}

/// Computed fields are computed and stored.
#[test]
fn test_computed_fields_reach_the_database() -> Result<()> {
    let app = app_or_skip!("t_compute");

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("price", 6);
    map.insert("amount", 7);
    let line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
    assert_eq!(*line.get_total_price(&mut env)?, 42);
    env.close()?;

    let mut env = app.new_env()?;
    let found: SaleOrderLine<MultipleIds> =
        env.search(&make_domain!([("total_price", "=", 42)]))?;
    assert_eq!(found.id.get_ids_ref().len(), 1);
    Ok(())
}

/// The dotted-path translation must agree with the in-memory backend, including the case where
/// two different children satisfy the two branches.
#[test]
fn test_dotted_path_search() -> Result<()> {
    let app = app_or_skip!("t_path");

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "order");
    let order: SaleOrder<SingleId> = env.create_new_record_from_map(map)?;
    for price in [10, 49, 10000] {
        let mut map: MapOfFields = MapOfFields::new(HashMap::new());
        map.insert("price", price);
        map.insert("amount", 1);
        map.insert("order", order.get_id());
        let _line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
    }
    env.close()?;

    let mut env = app.new_env()?;
    let matched: SaleOrder<MultipleIds> = env.search(&make_domain!([
        "&",
        ("lines.price", ">=", 30),
        ("lines.price", "<=", 50)
    ]))?;
    assert_eq!(
        matched.id.get_ids_ref().len(),
        1,
        "two different lines satisfy the two branches"
    );

    let none: SaleOrder<MultipleIds> = env.search(&make_domain!([("lines.price", "=", 77)]))?;
    assert!(none.id.is_empty());
    Ok(())
}

/// Deleting detaches children and removes the row.
#[test]
fn test_unlink_against_postgres() -> Result<()> {
    let app = app_or_skip!("t_unlink");

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "doomed");
    let order: SaleOrder<SingleId> = env.create_new_record_from_map(map)?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("amount", 5);
    map.insert("order", order.get_id());
    let line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
    env.close()?;

    let mut env = app.new_env()?;
    assert_eq!(env.delete("sale_order", &order.id)?, 1);
    env.close()?;

    let mut env = app.new_env()?;
    assert_eq!(
        env.count("sale_order", &make_domain!([("name", "=", "doomed")]))?,
        0
    );
    let rows = env.read("sale_order_line", &line.id, &["order"])?;
    assert!(
        rows[0].get_option::<&u32>("order").is_none(),
        "the child's foreign key must have been cleared"
    );
    Ok(())
}

/// Ordering and paging are emitted as SQL, not applied afterwards.
#[test]
fn test_order_limit_and_offset() -> Result<()> {
    let app = app_or_skip!("t_paging");

    let mut env = app.new_env()?;
    for name in ["delta", "alpha", "charlie", "bravo"] {
        let mut map: MapOfFields = MapOfFields::new(HashMap::new());
        map.insert("name", name);
        env.create_records("invoice", vec![map])?;
    }
    env.close()?;

    let mut env = app.new_env()?;
    let page = env.read_matching(
        "invoice",
        &["name"],
        &make_domain!([]),
        &SearchOptions::new()
            .order_by(OrderBy::asc("name"))
            .with_limit(2),
    )?;
    let names: Vec<String> = page
        .iter()
        .map(|row| row.get::<&String>("name").clone())
        .collect();
    assert_eq!(names, vec!["alpha", "bravo"]);

    assert_eq!(
        env.count("invoice", &make_domain!([]))?,
        4,
        "count ignores the limit"
    );
    Ok(())
}

/// The text operators map onto LIKE and ILIKE.
#[test]
fn test_text_operators() -> Result<()> {
    let app = app_or_skip!("t_text");

    let mut env = app.new_env()?;
    for name in ["Facture", "facture", "Brouillon"] {
        let mut map: MapOfFields = MapOfFields::new(HashMap::new());
        map.insert("name", name);
        env.create_records("invoice", vec![map])?;
    }
    env.close()?;

    let mut env = app.new_env()?;
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "like", "Fac%")]))?,
        1
    );
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "ilike", "FAC%")]))?,
        2
    );
    let wanted = vec!["Facture", "Brouillon"];
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "in", wanted)]))?,
        2
    );
    Ok(())
}

/// An empty optional field is a real NULL, and reads back as absent.
#[test]
fn test_null_round_trip() -> Result<()> {
    let app = app_or_skip!("t_null");

    let mut env = app.new_env()?;
    let ids = env.create_records("invoice", vec![MapOfFields::new(HashMap::new())])?;
    env.close()?;

    let mut env = app.new_env()?;
    let rows = env.read("invoice", &ids, &["signed_on", "name"])?;
    assert!(
        rows[0].get_option::<&NaiveDate>("signed_on").is_none(),
        "an optional field with no default must be NULL in the database"
    );
    assert_eq!(
        env.count(
            "invoice",
            &make_domain!([("signed_on", "=", None::<NaiveDate>)])
        )?,
        1,
        "an empty date must be reachable as NULL"
    );
    assert_eq!(
        env.count(
            "invoice",
            &make_domain!([("signed_on", "!=", None::<NaiveDate>)])
        )?,
        0
    );
    Ok(())
}

/// The relation table is created alongside the models that declare it.
#[test]
fn test_relation_table_is_created() -> Result<()> {
    let app = app_or_skip!("t_m2m_ddl");
    let mut database = app.create_new_database()?;
    let DatabaseType::Postgres(connection) = &mut database else {
        unreachable!()
    };

    let columns: Vec<String> = connection
        .client
        .query(
            "SELECT column_name FROM information_schema.columns \
             WHERE table_schema = 't_m2m_ddl' AND table_name = 'invoice_tag_rel' \
             ORDER BY column_name",
            &[],
        )?
        .iter()
        .map(|row| row.get(0))
        .collect();
    assert_eq!(columns, vec!["invoice_id", "tag_id"]);

    let invoice_columns: Vec<String> = connection
        .client
        .query(
            "SELECT column_name FROM information_schema.columns \
             WHERE table_schema = 't_m2m_ddl' AND table_name = 'invoice'",
            &[],
        )?
        .iter()
        .map(|row| row.get(0))
        .collect();
    assert!(
        !invoice_columns.contains(&"tags".to_string()),
        "a many2many has a table, not a column"
    );
    Ok(())
}

/// Pairs round-trip through the relation table, and are visible from both sides.
#[test]
fn test_many2many_against_postgres() -> Result<()> {
    let app = app_or_skip!("t_m2m");

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "INV");
    let invoices: MultipleIds = env.create_records("invoice", vec![map])?;
    let invoice = *invoices.get_ids_ref().first().unwrap();

    let mut tags = Vec::new();
    for name in ["urgent", "late"] {
        let mut map: MapOfFields = MapOfFields::new(HashMap::new());
        map.insert("name", name);
        let created: MultipleIds = env.create_records("tag", vec![map])?;
        tags.push(*created.get_ids_ref().first().unwrap());
    }

    let mut link: MapOfFields = MapOfFields::new(HashMap::new());
    link.insert("tags", tags.clone());
    env.write("invoice", &SingleId::from(invoice), link)?;
    env.close()?;

    let mut env = app.new_env()?;
    let rows = env.read("invoice", &SingleId::from(invoice), &["tags"])?;
    assert_eq!(
        rows[0]
            .get_option::<&Vec<u32>>("tags")
            .cloned()
            .unwrap_or_default(),
        tags,
        "the pairs must survive a commit"
    );

    let rows = env.read("tag", &SingleId::from(tags[0]), &["invoices"])?;
    assert_eq!(
        rows[0]
            .get_option::<&Vec<u32>>("invoices")
            .cloned()
            .unwrap_or_default(),
        vec![invoice],
        "and be visible from the other side"
    );

    // Dropping one target leaves the other alone.
    let mut link: MapOfFields = MapOfFields::new(HashMap::new());
    link.insert("tags", vec![tags[0]]);
    env.write("invoice", &SingleId::from(invoice), link)?;
    env.close()?;

    let mut env = app.new_env()?;
    let rows = env.read("tag", &SingleId::from(tags[1]), &["invoices"])?;
    assert!(
        rows[0]
            .get_option::<&Vec<u32>>("invoices")
            .cloned()
            .unwrap_or_default()
            .is_empty(),
        "the dropped tag must no longer see the invoice"
    );
    Ok(())
}

/// Values reach the database as bound parameters, never as SQL text.
///
/// Each attempt below would drop a table if it were interpolated; the table is checked
/// afterwards.
#[test]
fn test_values_cannot_inject_sql() -> Result<()> {
    let app = app_or_skip!("t_inject");

    let hostile = "'); DROP TABLE \"invoice\"; --";
    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", hostile);
    env.create_records("invoice", vec![map])?;
    env.close()?;

    // Stored verbatim, and found verbatim.
    let mut env = app.new_env()?;
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "=", hostile)]))?,
        1
    );
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "like", hostile)]))?,
        1
    );
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "!=", hostile)]))?,
        0
    );

    let members = vec![hostile, "harmless"];
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "in", members)]))?,
        1
    );
    Ok(())
}

/// The table is still there after all of that.
#[test]
fn test_injection_attempts_leave_the_schema_intact() -> Result<()> {
    let app = app_or_skip!("t_inject_schema");

    let mut env = app.new_env()?;
    for hostile in [
        "'; DROP TABLE \"invoice\"; --",
        "\" ; DROP TABLE \"invoice\" ; --",
        "1 OR 1=1",
        "\\'; DELETE FROM \"invoice\"; --",
    ] {
        let mut map: MapOfFields = MapOfFields::new(HashMap::new());
        map.insert("name", hostile);
        env.create_records("invoice", vec![map])?;
    }
    env.close()?;

    let mut env = app.new_env()?;
    assert_eq!(
        env.count("invoice", &make_domain!([]))?,
        4,
        "every hostile string was stored as data"
    );

    let mut database = app.create_new_database()?;
    let DatabaseType::Postgres(connection) = &mut database else {
        unreachable!()
    };
    let tables: i64 = connection
        .client
        .query_one(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema = 't_inject_schema' AND table_name = 'invoice'",
            &[],
        )?
        .get(0);
    assert_eq!(tables, 1, "the table must still exist");
    Ok(())
}

/// A hostile model or field name is refused by the registry before any SQL is built.
#[test]
fn test_hostile_names_are_refused() -> Result<()> {
    let app = app_or_skip!("t_inject_names");
    let mut env = app.new_env()?;

    let hostile_model = "invoice\"; DROP TABLE \"invoice\"; --";
    assert!(
        env.search_ids(hostile_model, &make_domain!([])).is_err(),
        "an unknown model must be refused"
    );

    let ids = env.create_records("invoice", vec![MapOfFields::new(HashMap::new())])?;
    let hostile_field = "name\"; DROP TABLE \"invoice\"; --";
    assert!(
        env.read("invoice", &ids, &[hostile_field]).is_err(),
        "an unknown field must be refused"
    );
    assert!(
        env.count("invoice", &make_domain!([(hostile_field, "=", "x")]))
            .is_err(),
        "and so must one inside a domain"
    );
    Ok(())
}

/// A sort key that is not a declared field is refused rather than reaching the database.
#[test]
fn test_hostile_order_is_refused() -> Result<()> {
    let app = app_or_skip!("t_inject_order");
    let mut env = app.new_env()?;

    let options = SearchOptions::new().order_by(OrderBy::asc("name\"; DROP TABLE \"invoice\"; --"));
    assert!(
        env.search_ids_with("invoice", &make_domain!([]), &options)
            .is_err(),
        "an unknown sort key must be refused"
    );
    Ok(())
}
