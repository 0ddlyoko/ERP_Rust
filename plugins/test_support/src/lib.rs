//! What the tests of the plugins share: an application installed on a database of its own, and
//! environments for the people working in it.
//!
//! The database is PostgreSQL when a server answers — the one the tests of `erp` use, in a schema
//! named after the test and wiped on the way in — and the in-memory database otherwise, or when
//! `ERP_TEST_MEMORY` is set.

use erp::Result;
use erp::app::Application;
use erp::config::Config;
use erp::data;
use erp::database::cache::CacheDatabase;
use erp::database::{DatabaseConfig, DatabaseType};
use erp::environment::Environment;
use erp::plugin::Plugin;
use erp::types::field::{Decimal, FieldType, IdMode};
use erp::types::model::MapOfFields;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::str::FromStr;
use std::sync::{Arc, Condvar, Mutex, PoisonError};

/// A decimal written in the test, `d("12.50")`.
pub fn d(value: &str) -> Decimal {
    Decimal::from_str(value).unwrap_or_else(|_| panic!("{value} is not a decimal"))
}

/// The schema a test works in: its name, shortened and made unique when too long for PostgreSQL.
fn schema_for(test: &str) -> String {
    let clean: String = test
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let mut hasher = DefaultHasher::new();
    test.hash(&mut hasher);
    let hash = format!("{:x}", hasher.finish());
    let keep = clean.len().min(40);
    format!("t_{}_{}", &clean[clean.len() - keep..], &hash[..8])
}

fn postgres_config(schema: &str) -> Config {
    let plugins = std::env::temp_dir().join("erp_test_support_plugins");
    std::fs::create_dir_all(&plugins).expect("a directory to scan for plugins");
    Config {
        database: DatabaseConfig {
            url: std::env::var("ERP_TEST_PGHOST").unwrap_or("/var/run/postgresql".to_string()),
            port: 5432,
            name: std::env::var("ERP_TEST_PGDATABASE").unwrap_or("erp_rust_test".to_string()),
            schema: schema.to_string(),
            user: std::env::var("PGUSER")
                .or_else(|_| std::env::var("USER"))
                .unwrap_or_default(),
            password: std::env::var("PGPASSWORD").unwrap_or_default(),
            pool_size: 3,
            connection_timeout: 10,
            revalidate_after: 0,
        },
        plugin_path: plugins.to_string_lossy().into_owned(),
        server: Default::default(),
    }
}

/// An application on PostgreSQL in a wiped schema, `None` when no server answers.
fn postgres_app(schema: &str) -> Option<Application> {
    let app = Application::new(postgres_config(schema));
    let mut database = app.create_new_database().ok()?;
    let DatabaseType::Postgres(connection) = &mut database else {
        return None;
    };
    connection
        .client
        .batch_execute(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
        .ok()?;
    drop(database);
    Some(app)
}

/// Whether this run uses PostgreSQL: a server answers and `ERP_TEST_MEMORY` is not set.
pub fn on_postgres() -> bool {
    std::env::var_os("ERP_TEST_MEMORY").is_none() && postgres_app("t_probe_connection").is_some()
}

/// The databases installed once per test binary, by what they hold: copied for each test in
/// memory, a schema each test works in, inside a transaction undone at its end, on PostgreSQL.
static INSTALLED: Mutex<Vec<(String, Installed)>> = Mutex::new(Vec::new());

#[derive(Clone)]
enum Installed {
    Memory(CacheDatabase),
    Postgres(String),
}

/// Whether a test on PostgreSQL is running in this process: one at a time, each in its
/// transaction on the shared schema, which another would wait on or deadlock with.
static RUNNING: Mutex<bool> = Mutex::new(false);
static TURN_ENDED: Condvar = Condvar::new();

/// A test's turn on PostgreSQL, given to the next once let go.
struct Turn;

impl Turn {
    fn take() -> Turn {
        let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        while *running {
            running = TURN_ENDED
                .wait(running)
                .unwrap_or_else(PoisonError::into_inner);
        }
        *running = true;
        Turn
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        *RUNNING.lock().unwrap_or_else(PoisonError::into_inner) = false;
        TURN_ENDED.notify_one();
    }
}

/// An application with the plugins `plugins` makes registered and those of `install` installed,
/// with their dependencies.
///
/// They are installed once per test binary: in memory, each test starts from a copy of that
/// database; on PostgreSQL, each works in its schema inside one transaction, rolled back once
/// the application — and those succeeding it — are let go, the tests of the binary running one
/// after another. What a test does to the database is thus never seen by another, without paying
/// for an installation each time. A test needing several connections at once — locks, concurrent
/// transactions — takes a [`committing_app`] instead.
pub fn app(plugins: impl Fn() -> Vec<Box<dyn Plugin>>, install: &[&str]) -> Result<Application> {
    let installed = installed(&plugins, install)?;
    let mut app = match installed {
        Installed::Memory(database) => {
            let mut app = Application::new_test();
            app.cache_db = database.copy();
            app
        }
        Installed::Postgres(schema) => {
            let turn = Turn::take();
            let mut app = Application::new(postgres_config(&schema));
            let transaction = app.pin_transaction()?;
            app.hold(Arc::new(transaction));
            app.hold(Arc::new(turn));
            app
        }
    };
    for plugin in plugins() {
        app.register_plugin(plugin)?;
    }
    app.load()?;
    Ok(app)
}

/// The database holding `install` for this binary, installed the first time it is asked for.
fn installed(plugins: &impl Fn() -> Vec<Box<dyn Plugin>>, install: &[&str]) -> Result<Installed> {
    let mut names: Vec<String> = plugins().iter().map(|plugin| plugin.name()).collect();
    names.sort();
    let key = format!("{}|{}", install.join(","), names.join(","));
    let mut databases = INSTALLED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, installed)) = databases.iter().find(|(known, _)| *known == key) {
        return Ok(installed.clone());
    }
    let binary = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    let postgres = if std::env::var_os("ERP_TEST_MEMORY").is_some() {
        None
    } else {
        let schema = schema_for(&format!("{binary}::{key}"));
        postgres_app(&schema).map(|app| (app, schema))
    };
    let (mut app, made) = match postgres {
        Some((app, schema)) => (app, Installed::Postgres(schema)),
        None => {
            let app = Application::new_test();
            let database = app.cache_db.clone();
            (app, Installed::Memory(database))
        }
    };
    for plugin in plugins() {
        app.register_plugin(plugin)?;
    }
    if let Installed::Postgres(_) = made {
        app.load()?;
    }
    for name in install {
        app.load_plugin(name)?;
    }
    databases.push((key, made.clone()));
    Ok(made)
}

/// An application with `plugins` registered and `install` installed, with its dependencies, on a
/// database of its own whose transactions commit: for a test of what several connections do at
/// once. On PostgreSQL the schema is named after the running test, so tests run side by side.
pub fn committing_app(plugins: Vec<Box<dyn Plugin>>, install: &str) -> Result<Application> {
    let test = std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .to_string();
    let postgres = if std::env::var_os("ERP_TEST_MEMORY").is_some() {
        None
    } else {
        postgres_app(&schema_for(&test))
    };
    let mut app = match postgres {
        Some(mut app) => {
            for plugin in plugins {
                app.register_plugin(plugin)?;
            }
            app.load()?;
            app
        }
        None => {
            let mut app = Application::new_test();
            for plugin in plugins {
                app.register_plugin(plugin)?;
            }
            app
        }
    };
    app.load_plugin(install)?;
    Ok(app)
}

/// The id of the record `xml_id` names; panics when it names none, as a broken test should.
pub fn xml_id(env: &mut Environment, xml_id: &str) -> u32 {
    data::resolve(env, xml_id)
        .unwrap_or_else(|error| panic!("resolving {xml_id}: {error}"))
        .unwrap_or_else(|| panic!("{xml_id} names no record"))
}

/// The administrator, who may do anything.
pub fn admin_env(app: &Application) -> Result<Environment<'_>> {
    let admin = {
        let mut env = app.new_env_as_option(None)?;
        xml_id(&mut env, "base.user_admin")
    };
    app.new_env_as_option(Some(admin))
}

/// A new user in `groups` (external ids), committed so other environments see them.
pub fn new_user(app: &Application, login: &str, groups: &[&str]) -> Result<u32> {
    let mut env = admin_env(app)?;
    let groups = groups.iter().map(|group| xml_id(&mut env, group)).collect();
    let mut values = MapOfFields::default();
    values.insert("login", login);
    values.insert("name", login);
    values.insert("groups", FieldType::Refs(groups));
    let uid = env.create_records("users", vec![values])?.get_ids_ref()[0];
    env.close()?;
    Ok(uid)
}

/// An environment as a new user of `groups`.
pub fn user_env<'a>(app: &'a Application, login: &str, groups: &[&str]) -> Result<Environment<'a>> {
    let uid = new_user(app, login, groups)?;
    app.new_env_as_option(Some(uid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_names_fit_postgres() {
        let long = "a::very::long::module::path::test_with_a_name_much_longer_than_needed";
        let schema = schema_for(long);
        assert!(schema.len() <= 63, "{schema}");
        assert!(schema.starts_with("t_"));
        assert_ne!(schema_for("x::test_a"), schema_for("y::test_a"));
        assert_eq!(schema_for("Test-1"), schema_for("Test-1"));
    }
}
