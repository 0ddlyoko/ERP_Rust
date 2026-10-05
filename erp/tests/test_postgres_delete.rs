//! Deleting records holding a required many2one, against a real PostgreSQL server, whose NOT NULL
//! constraints the in-memory database does not have.
//!
//! Skipped, not failed, when no server is reachable.
use base::BasePlugin;
use erp::app::Application;
use erp::config::Config;
use erp::data;
use erp::database::{DatabaseConfig, DatabaseType};
use erp_types::field::{FieldType, IdMode, SingleId};
use erp_types::model::MapOfFields;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn config(schema: &str) -> Config {
    let plugins = std::env::temp_dir().join("erp_postgres_delete_plugins");
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
            pool_size: 3,
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

/// A user's contact is required. Deleting the user empties its relational fields on the way
/// out, for the lists mirroring them; that must not reach the row, whose column is NOT NULL.
#[test]
fn test_deleting_a_record_with_a_required_many2one() -> Result<()> {
    let Some(app) = base_on_postgres("t_delete_required") else {
        eprintln!("skipping: no PostgreSQL server reachable");
        return Ok(());
    };
    let admin = admin(&app)?;
    let user = {
        let mut env = app.new_env_as_option(Some(admin))?;
        let group = data::resolve(&mut env, "base.group_user")?.expect("seeded");
        let mut values = MapOfFields::default();
        values.insert("login", "leaving");
        values.insert("name", "Leaving");
        values.insert("groups", FieldType::Refs(vec![group]));
        let user = env.create_records("users", vec![values])?.get_ids_ref()[0];
        env.close()?;
        user
    };

    let mut env = app.new_env_as_option(Some(admin))?;
    let deleted = env.delete("users", &SingleId::from(user))?;
    assert_eq!(deleted, 1);
    env.close()?;

    let mut env = app.new_env_as_option(None)?;
    let remaining = env.sudo().count(
        "users",
        &erp_search_code_gen::make_domain!([("login", "=", "leaving")]),
    )?;
    assert_eq!(remaining, 0);
    Ok(())
}

/// The same for a record created in the same unit of work, never written to the server before.
#[test]
fn test_deleting_a_record_created_in_the_same_transaction() -> Result<()> {
    let Some(app) = base_on_postgres("t_delete_required_new") else {
        eprintln!("skipping: no PostgreSQL server reachable");
        return Ok(());
    };
    let mut env = app.new_env_as_option(Some(admin(&app)?))?;
    let mut values = MapOfFields::default();
    values.insert("login", "brief");
    values.insert("name", "Brief");
    let user = env.create_records("users", vec![values])?;
    assert_eq!(env.delete("users", &user)?, 1);
    env.close()?;
    Ok(())
}
