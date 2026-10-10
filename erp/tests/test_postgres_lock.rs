//! Locking records against a real PostgreSQL server: a transaction locking rows another holds
//! waits for it, then reads what it committed.
//!
//! Skipped, not failed, when no server is reachable.
use base::BasePlugin;
use base::models::Contact;
use erp::Result;
use erp::app::Application;
use erp::config::Config;
use erp::data;
use erp::database::{DatabaseConfig, DatabaseType};
use erp_types::field::SingleId;
use erp_types::model::MapOfFields;
use std::sync::Barrier;
use std::time::Duration;

fn config(schema: &str) -> Config {
    let plugins = std::env::temp_dir().join("erp_postgres_lock_plugins");
    std::fs::create_dir_all(&plugins).expect("a directory to scan");
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
            pool_size: 4,
            connection_timeout: 10,
            revalidate_after: 0,
        },
        plugin_path: plugins.to_string_lossy().into_owned(),
        server: erp::server_config::ServerConfig::default(),
    }
}

/// `base` loaded on a wiped schema, `None` without a server.
fn base_on_postgres(schema: &str) -> Option<Application> {
    let mut app = Application::new(config(schema));
    let mut database = app.create_new_database().ok()?;
    let DatabaseType::Postgres(connection) = &mut database else {
        return None;
    };
    connection
        .client
        .batch_execute(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
        .ok()?;
    drop(database);
    app.register_plugin(Box::new(BasePlugin {})).ok()?;
    app.load().expect("loading base against PostgreSQL");
    Some(app)
}

fn admin(app: &Application) -> Result<u32> {
    let mut env = app.new_env_as_option(None)?;
    Ok(data::resolve(&mut env, "base.user_admin")?.expect("seeded"))
}

/// Append `suffix` to the contact's name, holding it locked, the way a counter moves on.
fn append(
    app: &Application,
    contact: u32,
    suffix: &str,
    lock: bool,
    hold: Option<&Barrier>,
) -> Result<()> {
    let mut env = app.new_env_as_option(Some(admin(app)?))?;
    let record: Contact<SingleId> = env.get_record(contact.into());
    if lock {
        env.lock_records("contact", &SingleId::from(contact))?;
    }
    let name = record.get_name(&mut env)?.clone();
    if let Some(barrier) = hold {
        // The other transaction starts now, and must wait for this one to commit.
        barrier.wait();
        std::thread::sleep(Duration::from_millis(300));
    }
    let mut values = MapOfFields::default();
    values.insert("name", format!("{name}-{suffix}"));
    env.write("contact", &SingleId::from(contact), values)?;
    env.close()
}

fn new_contact(app: &Application) -> Result<u32> {
    let mut env = app.new_env_as_option(Some(admin(app)?))?;
    let mut values = MapOfFields::default();
    values.insert("name", "Start");
    let contact: Contact<SingleId> = env.create_new_record_from_map(values)?;
    env.close()?;
    Ok(contact.get_id())
}

fn name_of(app: &Application, contact: u32) -> Result<String> {
    let mut env = app.new_env_as_option(Some(admin(app)?))?;
    let record: Contact<SingleId> = env.get_record(contact.into());
    Ok(record.get_name(&mut env)?.clone())
}

/// Two transactions moving the same record on, both locking it: the second waits for the first
/// and builds on what it committed.
#[test]
fn test_a_locked_record_is_read_after_the_other_transaction() -> Result<()> {
    let Some(app) = base_on_postgres("t_lock_records") else {
        eprintln!("skipping: no PostgreSQL server reachable");
        return Ok(());
    };
    let contact = new_contact(&app)?;
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| -> Result<()> {
        let first = scope.spawn(|| append(&app, contact, "A", true, Some(&barrier)));
        barrier.wait();
        append(&app, contact, "B", true, None)?;
        first.join().expect("the first transaction")?;
        Ok(())
    })?;
    assert_eq!(name_of(&app, contact)?, "Start-A-B");
    Ok(())
}

/// Without locking, the second transaction reads what was there before the first committed, and
/// overwrites it: what locking is for.
#[test]
fn test_without_locking_a_change_is_lost() -> Result<()> {
    let Some(app) = base_on_postgres("t_lock_records_none") else {
        eprintln!("skipping: no PostgreSQL server reachable");
        return Ok(());
    };
    let contact = new_contact(&app)?;
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| -> Result<()> {
        let first = scope.spawn(|| append(&app, contact, "A", false, Some(&barrier)));
        barrier.wait();
        let second = append(&app, contact, "B", false, None);
        first.join().expect("the first transaction")?;
        // The second may also fail on the row the first holds; either way, A's change is lost
        // or B's is refused — never both kept.
        if second.is_ok() {
            assert_ne!(name_of(&app, contact)?, "Start-A-B");
        }
        Ok(())
    })?;
    Ok(())
}
