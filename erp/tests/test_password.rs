//! Secrets stored hashed.
//!
//! A `Password` is built from a clear password, which it hashes and then forgets. Storing one
//! stores the hash; reading one back gives the hash — and not even the hash comes out of it. What
//! a caller can do is ask whether something matches.
//!
//! Distinct from a merely private field, which the model beside it carries: `unlock_code` is kept
//! from the wire but readable in full by the program, while `service_key` is readable by nobody.

use erp::app::Application;
use erp::config::Config;
use erp::database::{Database, DatabaseConfig, DatabaseType, FieldType as StoredType};
use erp::jsonrpc;
use erp_search::RightTuple;
use erp_search_code_gen::make_domain;
use erp_types::field::{FieldKind, FieldType, IdMode, MultipleIds, Password};
use erp_types::model::MapOfFields;
use serde_json::{Value, json};
use std::error::Error;
use test_plugin::TestPlugin;
use test_utilities::TestLibPlugin;
use test_utilities::models::Machine;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(TestPlugin {}))?;
    app.load_plugin("test_plugin")?;
    Ok(app)
}

/// The hash as storage sees it.
///
/// Nothing on `Password` reads it; the conversion into a stored value is where it becomes a
/// string, because a column has to hold something. That is the boundary these tests watch.
fn stored_hash(password: &Password) -> String {
    let StoredType::Password(hash) = StoredType::from(FieldType::Password(password.clone())) else {
        panic!("a password stores as a password");
    };
    hash
}

/// A machine with a service key, made from inside the process.
///
/// Not over the wire: a hidden field cannot be written from outside, which is what
/// `test_writing_a_password_over_the_wire_is_refused` is about.
fn make(app: &Application, name: &str, key: &str) -> Result<Vec<u32>> {
    let mut env = app.new_env()?;
    let mut values = MapOfFields::default();
    values.insert("name", name);
    values.insert("service_key", Password::new(key)?);
    let ids: MultipleIds = env.create_records("machine", vec![values])?;
    let ids = ids.get_ids_ref().clone();
    env.close()?;
    Ok(ids)
}

fn raw(app: &Application, method: &str, params: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).to_string();
    jsonrpc::handle(app, None, &body).expect("an answer")
}

fn call(app: &Application, method: &str, params: Value) -> Value {
    let answer = raw(app, method, params);
    assert!(
        answer.get("error").is_none(),
        "{method} must not fail: {answer}"
    );
    answer["result"].clone()
}

// ---- the type itself ----

/// Two accounts sharing a password do not share a hash.
#[test]
fn test_each_password_carries_its_own_salt() -> Result<()> {
    let first = Password::new("hunter2")?;
    let second = Password::new("hunter2")?;

    assert_ne!(first, second);
    assert!(first.is_same_password("hunter2"));
    assert!(second.is_same_password("hunter2"));
    Ok(())
}

/// What is kept is the hash, and the clear password is nowhere in it.
#[test]
fn test_the_clear_password_is_not_in_what_is_stored() -> Result<()> {
    let hash = stored_hash(&Password::new("hunter2")?);

    assert!(!hash.contains("hunter2"), "got {hash}");
    assert!(hash.starts_with("$argon2"), "a PHC string: {hash}");
    Ok(())
}

/// The clear password is answered, never returned.
#[test]
fn test_is_same_password() -> Result<()> {
    let password = Password::new("hunter2")?;

    assert!(password.is_same_password("hunter2"));
    assert!(!password.is_same_password("hunter3"));
    assert!(!password.is_same_password(""));
    Ok(())
}

/// So is a hash somebody already holds, which is how two stored values are compared without
/// either being read.
#[test]
fn test_is_same_hash() -> Result<()> {
    let password = Password::new("hunter2")?;
    let hash = stored_hash(&password);

    assert!(password.is_same_hash(&hash));
    assert!(!password.is_same_hash("not the same"));
    assert!(!password.is_same_hash(&stored_hash(&Password::new("hunter2")?)));
    Ok(())
}

/// Neither form of printing gives anything away — a log line cannot leak what it never held.
#[test]
fn test_a_password_prints_as_a_mask() -> Result<()> {
    let password = Password::new("hunter2")?;

    assert_eq!(format!("{password}"), "****");
    assert_eq!(format!("{password:?}"), "Password(****)");
    assert_eq!(format!("{}", FieldType::Password(password.clone())), "****");
    assert_eq!(
        format!("{:?}", FieldType::Password(password)),
        "Password(Password(****))"
    );
    Ok(())
}

/// Serialized, a password is nothing at all — the value itself refuses, without relying on a
/// caller having been filtered first.
#[test]
fn test_a_password_serializes_to_nothing() -> Result<()> {
    let value = FieldType::Password(Password::new("hunter2")?);

    assert_eq!(serde_json::to_string(&value)?, "null");
    Ok(())
}

/// A stored password equals nothing a domain can hold, down at the comparison itself — including
/// the empty value, which every other type answers by matching whatever list contains it.
#[test]
fn test_a_stored_password_equals_no_domain_value() {
    let stored = StoredType::Password("$argon2id$v=19$m=19456,t=2,p=1$salt$hash".to_string());

    assert!(stored != RightTuple::String("$argon2id$v=19$m=19456,t=2,p=1$salt$hash".to_string()));
    assert!(stored != RightTuple::None);
    assert!(stored != RightTuple::Array(vec![RightTuple::None]));
}

/// A record created without a password has one that matches nothing, not one that matches
/// everything.
#[test]
fn test_a_password_never_set_matches_nothing() {
    let password = Password::default();

    assert!(!password.is_set());
    assert!(!password.is_same_password(""));
    assert!(!password.is_same_password("anything"));
}

/// A hash that cannot be read is an answer of no, not a crash.
#[test]
fn test_a_malformed_hash_matches_nothing() {
    assert!(!Password::from_hash("not-a-hash").is_same_password("anything"));
}

/// Text destined for a password field is the clear password, and lands hashed. This is the route
/// a data file takes.
#[test]
fn test_text_is_read_as_a_clear_password() -> Result<()> {
    let FieldType::Password(password) = FieldKind::Password.parse("hunter2")? else {
        panic!("a password");
    };

    assert!(password.is_same_password("hunter2"));
    assert!(!stored_hash(&password).contains("hunter2"));
    Ok(())
}

// ---- through the ORM ----

/// The hash reaches storage and comes back able to verify, through an environment that never saw
/// it written.
#[test]
fn test_a_password_survives_a_round_trip() -> Result<()> {
    let app = new_app()?;

    let ids: MultipleIds = {
        let mut env = app.new_env()?;
        let mut values = MapOfFields::default();
        values.insert("name", "m");
        values.insert("service_key", Password::new("hunter2")?);
        let ids = env.create_records("machine", vec![values])?;
        env.close()?;
        ids
    };

    let mut env = app.new_env()?;
    let machine: Machine<MultipleIds> = env.get_record(ids);
    let stored = machine.get_service_key(&mut env)?;
    assert_eq!(stored.len(), 1);
    assert!(stored[0].is_same_password("hunter2"));
    assert!(!stored[0].is_same_password("hunter3"));
    Ok(())
}

/// Declaring the type is enough: nothing has to remember to mark it private.
#[test]
fn test_a_password_field_is_private_without_being_told() -> Result<()> {
    let app = new_app()?;
    let machine = app.model_manager.get_model("machine");

    let key = machine.get_internal_field("service_key");
    assert_eq!(key.kind, FieldKind::Password);
    assert!(key.private, "a password is hidden by its type alone");
    Ok(())
}

// ---- through the API ----

/// Setting one from outside is refused rather than dropped.
///
/// Reading a hidden field is answered empty, because the caller gets the shape it asked for.
/// Writing cannot be answered that way: a value silently ignored would report a change that never
/// happened, and nothing the caller can read afterwards would say otherwise.
#[test]
fn test_writing_a_password_over_the_wire_is_refused() -> Result<()> {
    let app = new_app()?;

    let created = raw(
        &app,
        "machine.create",
        json!({"values": {"name": "m", "service_key": "hunter2"}}),
    );
    assert!(
        created["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("service_key")),
        "got {created}"
    );

    let ids = make(&app, "m", "hunter2")?;
    let written = raw(
        &app,
        "machine.write",
        json!({"ids": ids, "values": {"service_key": "second"}}),
    );
    assert!(written.get("result").is_none(), "got {written}");

    let mut env = app.new_env()?;
    let machine: Machine<MultipleIds> = env.get_record(MultipleIds::from(ids));
    assert!(
        machine.get_service_key(&mut env)?[0].is_same_password("hunter2"),
        "the refused write changed nothing"
    );
    Ok(())
}

/// The same refusal reaches a field that is private without being a password.
#[test]
fn test_writing_any_private_field_is_refused() -> Result<()> {
    let app = new_app()?;
    let created = raw(
        &app,
        "machine.create",
        json!({"values": {"name": "m", "unlock_code": "s3cret"}}),
    );

    assert!(
        created["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("unlock_code")),
        "got {created}"
    );
    Ok(())
}

/// Reading it back gives nothing, like any other hidden field.
#[test]
fn test_reading_a_password_gives_nothing() -> Result<()> {
    let app = new_app()?;
    let ids = make(&app, "m", "hunter2")?;

    let rows = call(
        &app,
        "machine.read",
        json!({"ids": ids, "fields": ["name", "service_key"]}),
    );
    assert_eq!(rows[0]["name"], json!("m"));
    assert_eq!(rows[0]["service_key"], Value::Null, "got {rows}");
    Ok(())
}

/// And no domain answers questions about it — including one that sends the hash itself, which is
/// the comparison that would rebuild it a character at a time.
#[test]
fn test_no_domain_matches_a_password() -> Result<()> {
    let app = new_app()?;
    let ids = make(&app, "m", "hunter2")?;
    let hash = {
        let mut env = app.new_env()?;
        let machine: Machine<MultipleIds> = env.get_record(MultipleIds::from(ids));
        stored_hash(machine.get_service_key(&mut env)?[0])
    };

    for domain in [
        json!([["service_key", "=", "hunter2"]]),
        json!([["service_key", "=", hash]]),
        json!([["service_key", "like", "$argon2%"]]),
    ] {
        let found = call(&app, "machine.search", json!({"domain": domain}));
        assert_eq!(found, json!([]), "matched on {domain}");
    }
    Ok(())
}

/// Not only over the wire: a domain written in Rust matches nothing either. The program holds
/// no more of a password than a caller does.
#[test]
fn test_a_domain_written_in_rust_matches_no_password() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    let mut values = MapOfFields::default();
    values.insert("name", "m");
    values.insert("service_key", Password::new("hunter2")?);
    let ids: MultipleIds = env.create_records("machine", vec![values])?;

    let machine: Machine<MultipleIds> = env.get_record(ids);
    let hash = stored_hash(machine.get_service_key(&mut env)?[0]);

    for domain in [
        make_domain!([("service_key", "=", hash.as_str())]),
        make_domain!([("service_key", "like", "$argon2%")]),
    ] {
        let found: Machine<MultipleIds> = env.search(&domain)?;
        assert!(found.get_ids().is_empty(), "matched on {domain:?}");
    }
    Ok(())
}

/// Sorting by one is dropped rather than obeyed, for the same reason.
#[test]
fn test_ordering_by_a_password_is_dropped() -> Result<()> {
    let app = new_app()?;
    make(&app, "m", "hunter2")?;

    let rows = call(
        &app,
        "machine.read_matching",
        json!({"domain": [], "fields": ["name"], "order": ["service_key asc"]}),
    );
    assert_eq!(rows.as_array().map(Vec::len), Some(1), "got {rows}");
    Ok(())
}

// ---- against a real server ----

const DATABASE: &str = "erp_rust_test";

fn postgres_app(schema: &str) -> Option<Application> {
    let config = Config {
        database: DatabaseConfig {
            url: std::env::var("ERP_TEST_PGHOST").unwrap_or("/var/run/postgresql".to_string()),
            port: 5432,
            name: std::env::var("ERP_TEST_PGDATABASE").unwrap_or(DATABASE.to_string()),
            schema: schema.to_string(),
            user: std::env::var("PGUSER")
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_default(),
            password: std::env::var("PGPASSWORD").unwrap_or_default(),
            pool_size: 10,
            connection_timeout: 10,
            revalidate_after: 0,
        },
        plugin_path: String::new(),
        server: erp::server_config::ServerConfig::default(),
    };

    let mut app = Application::new(config);
    app.model_manager.register_model::<Machine<_>>();
    app.model_manager.post_register();

    let mut database = app.create_new_database().ok()?;
    let DatabaseType::Postgres(connection) = &mut database else {
        return None;
    };
    connection
        .client
        .batch_execute(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
        .expect("a reachable server answers");
    database.initialize().expect("a schema of its own");
    let model = app.model_manager.get_model("machine");
    database.sync_model(model).expect("a table for the model");
    drop(database);
    Some(app)
}

/// The same refusal against the real server, where the comparison would otherwise be the
/// database's to make.
#[test]
fn test_no_domain_matches_a_password_in_postgres() -> Result<()> {
    let Some(app) = postgres_app("t_password_domain") else {
        eprintln!("skipping: no PostgreSQL server reachable");
        return Ok(());
    };

    let mut env = app.new_env()?;
    let mut values = MapOfFields::default();
    values.insert("name", "m");
    values.insert("service_key", Password::new("hunter2")?);
    let ids: MultipleIds = env.create_records("machine", vec![values])?;
    let machine: Machine<MultipleIds> = env.get_record(ids);
    let hash = stored_hash(machine.get_service_key(&mut env)?[0]);
    env.save_all_to_db()?;

    for domain in [
        make_domain!([("service_key", "=", hash.as_str())]),
        make_domain!([("service_key", "like", "$argon2%")]),
    ] {
        let found: Machine<MultipleIds> = env.search(&domain)?;
        assert!(found.get_ids().is_empty(), "matched on {domain:?}");
    }

    let found: Machine<MultipleIds> = env.search(&make_domain!([("name", "=", "m")]))?;
    assert_eq!(found.get_ids().len(), 1, "the record is there to be missed");
    Ok(())
}

/// The column holds the hash, and what comes back out of it still verifies.
#[test]
fn test_a_password_round_trips_through_postgres() -> Result<()> {
    let Some(app) = postgres_app("t_password") else {
        eprintln!("skipping: no PostgreSQL server reachable");
        return Ok(());
    };

    let ids: MultipleIds = {
        let mut env = app.new_env()?;
        let mut values = MapOfFields::default();
        values.insert("name", "m");
        values.insert("service_key", Password::new("hunter2")?);
        let ids = env.create_records("machine", vec![values])?;
        env.close()?;
        ids
    };

    let mut env = app.new_env()?;
    let machine: Machine<MultipleIds> = env.get_record(ids.clone());
    assert!(machine.get_service_key(&mut env)?[0].is_same_password("hunter2"));

    let mut database = app.create_new_database()?;
    let DatabaseType::Postgres(connection) = &mut database else {
        unreachable!()
    };
    let column: String = connection
        .client
        .query_one(
            "SELECT data_type FROM information_schema.columns \
             WHERE table_schema = 't_password' AND table_name = 'machine' \
             AND column_name = 'service_key'",
            &[],
        )?
        .get(0);
    assert_eq!(column, "text", "the hash lives in a text column");

    let stored: String = connection
        .client
        .query_one(
            "SELECT service_key FROM machine WHERE id = $1",
            &[&(ids.get_ids_ref()[0] as i32)],
        )?
        .get(0);
    assert!(stored.starts_with("$argon2"), "got {stored}");
    assert!(!stored.contains("hunter2"));
    Ok(())
}
