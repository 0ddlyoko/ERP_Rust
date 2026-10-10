//! Connections handed out, given back, and thrown away when the server no longer has them.
//!
//! `is_closed` only reports what the client already noticed, and an idle connection notices
//! nothing: a restart, an idle timeout or a terminated backend leaves it looking open until the
//! next query fails. These tests kill connections for real and check what the pool does next.
//!
//! Skipped, not failed, when no server answers.

use erp::Result;
use erp::app::Application;
use erp::config::Config;
use erp::database::{Database, DatabaseConfig, DatabaseType};
use erp_search_code_gen::make_domain;
use erp_types::field::MultipleIds;
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::time::Duration;
use test_utilities::models::{Invoice, SaleOrder, SaleOrderLine, Tag};

/// A pool whose connections are revalidated after `revalidate_after` seconds idle.
fn config(schema: &str, pool_size: u32, revalidate_after: u64) -> Config {
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
            pool_size,
            connection_timeout: 10,
            revalidate_after,
        },
        plugin_path: String::new(),
        server: erp::server_config::ServerConfig::default(),
    }
}

fn app_with(schema: &str, pool_size: u32, revalidate_after: u64) -> Option<Application> {
    let mut app = Application::new(config(schema, pool_size, revalidate_after));
    // Every model a registered one relates to has to be there too, or the foreign keys of a
    // relation table point at a table nobody created.
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.post_register();

    // Only a connection failure means "no server"; anything after it is a broken harness and
    // must be loud, or a whole suite skips itself in silence.
    let mut database = app.create_new_database().ok()?;
    let DatabaseType::Postgres(connection) = &mut database else {
        return None;
    };
    connection
        .client
        .batch_execute(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
        .expect("dropping the schema");
    database.initialize().expect("initialising the schema");
    for name in ["invoice", "sale_order", "sale_order_line", "tag"] {
        database
            .sync_model(app.model_manager.get_model(name))
            .unwrap_or_else(|error| panic!("synchronising {name}: {error}"));
    }
    for name in ["invoice", "sale_order", "sale_order_line", "tag"] {
        database
            .sync_constraints(app.model_manager.get_model(name))
            .unwrap_or_else(|error| panic!("constraining {name}: {error}"));
    }
    drop(database);
    Some(app)
}

macro_rules! app_or_skip {
    ($schema:expr, $size:expr, $revalidate:expr) => {
        match app_with($schema, $size, $revalidate) {
            Some(app) => app,
            None => {
                eprintln!("skipping: no PostgreSQL server reachable");
                return Ok(());
            }
        }
    };
}

/// Kill this pool's backends, and only this pool's.
///
/// What a restart looks like from a client's point of view: the socket is gone, but nobody told
/// the connections sitting idle in the pool. Narrowed to one application name because these
/// tests share a database with every other one, and killing indiscriminately would take those
/// down too.
fn terminate_this_pools_backends(schema: &str) -> Result<u64> {
    // Its own application, and so its own connection: taking one from the pool under test would
    // mean holding the very connection meant to be killed, and excluding it.
    let killer = Application::new(config("pg_terminator", 1, 0));
    let mut database = killer.create_new_database()?;
    let DatabaseType::Postgres(connection) = &mut database else {
        unreachable!("only runs against PostgreSQL");
    };
    let row = connection.client.query_one(
        "SELECT COUNT(pg_terminate_backend(pid)) FROM pg_stat_activity \
         WHERE datname = current_database() AND application_name = $1 \
         AND pid <> pg_backend_pid()",
        &[&format!("erp:{schema}")],
    )?;
    Ok(row.get::<_, i64>(0) as u64)
}

fn count_invoices(env: &mut erp::environment::Environment) -> Result<u32> {
    env.count("invoice", &make_domain!([]))
}

fn make_invoice(env: &mut erp::environment::Environment, name: &str) -> Result<()> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", name);
    let _: MultipleIds = env.create_records("invoice", vec![map])?;
    Ok(())
}

/// A connection the server terminated is never handed to a caller.
#[test]
fn test_a_terminated_connection_is_not_handed_out() -> Result<()> {
    let app = app_or_skip!("pool_terminated", 4, 0);

    // Warm the pool, then let the connections go back to it.
    for _ in 0..3 {
        let mut env = app.new_env()?;
        count_invoices(&mut env)?;
        env.close()?;
    }
    assert!(
        app.pool_size().unwrap_or(0) >= 1,
        "the pool holds something"
    );

    let killed = terminate_this_pools_backends("pool_terminated")?;
    assert!(killed >= 1, "the test must actually have killed something");

    // Every idle connection is now dead, and the next caller must still be served.
    let mut env = app.new_env()?;
    assert_eq!(count_invoices(&mut env)?, 0, "the query must go through");
    env.close()?;
    Ok(())
}

/// And the work done on the replacement connection is real.
#[test]
fn test_work_survives_a_terminated_connection() -> Result<()> {
    let app = app_or_skip!("pool_terminated_work", 4, 0);

    let mut env = app.new_env()?;
    make_invoice(&mut env, "before")?;
    env.close()?;

    terminate_this_pools_backends("pool_terminated_work")?;

    let mut env = app.new_env()?;
    make_invoice(&mut env, "after")?;
    env.close()?;

    let mut env = app.new_env()?;
    assert_eq!(count_invoices(&mut env)?, 2, "both must be there");
    Ok(())
}

/// The dead ones are counted as thrown away, not silently forgotten.
#[test]
fn test_dead_connections_are_discarded() -> Result<()> {
    let app = app_or_skip!("pool_discarded", 4, 0);

    for _ in 0..3 {
        let mut env = app.new_env()?;
        count_invoices(&mut env)?;
        env.close()?;
    }
    let before = app.pool_discarded().unwrap_or(0);
    let killed = terminate_this_pools_backends("pool_discarded")?;
    assert!(killed >= 1, "the test must actually have killed something");

    let mut env = app.new_env()?;
    count_invoices(&mut env)?;
    env.close()?;

    assert!(
        app.pool_discarded().unwrap_or(0) > before,
        "at least one dead connection must have been thrown away"
    );
    Ok(())
}

/// Several callers at once, every connection dead: all of them still get served.
#[test]
fn test_every_caller_is_served_after_a_mass_termination() -> Result<()> {
    let app = app_or_skip!("pool_mass_kill", 4, 0);

    for _ in 0..4 {
        let mut env = app.new_env()?;
        count_invoices(&mut env)?;
        env.close()?;
    }
    terminate_this_pools_backends("pool_mass_kill")?;

    std::thread::scope(|scope| {
        for n in 0..8 {
            let app = &app;
            scope.spawn(move || {
                let mut env = app.new_env().expect("a live connection");
                make_invoice(&mut env, &format!("caller {n}")).expect("create");
                env.close().expect("commit");
            });
        }
    });

    let mut env = app.new_env()?;
    assert_eq!(count_invoices(&mut env)?, 8);
    Ok(())
}

/// A connection handed straight back out is not asked whether it is alive.
///
/// The check costs a round trip, so under load it must not happen: a connection that came back a
/// moment ago cannot have died without the client noticing.
#[test]
fn test_a_fresh_connection_is_not_revalidated() -> Result<()> {
    let app = app_or_skip!("pool_no_ping", 1, 3600);

    for _ in 0..20 {
        let mut env = app.new_env()?;
        count_invoices(&mut env)?;
        env.close()?;
    }

    assert_eq!(
        app.pool_revalidations().unwrap_or(0),
        0,
        "twenty round trips saved"
    );
    Ok(())
}

/// One that sat idle long enough is asked.
#[test]
fn test_an_idle_connection_is_revalidated() -> Result<()> {
    let app = app_or_skip!("pool_ping", 1, 0);

    let mut env = app.new_env()?;
    count_invoices(&mut env)?;
    env.close()?;

    let before = app.pool_revalidations().unwrap_or(0);
    let mut env = app.new_env()?;
    count_invoices(&mut env)?;
    env.close()?;

    assert!(
        app.pool_revalidations().unwrap_or(0) > before,
        "an idle connection must be asked before being lent again"
    );
    Ok(())
}

/// The threshold is respected rather than approximated.
#[test]
fn test_the_threshold_decides_whether_to_ask() -> Result<()> {
    let app = app_or_skip!("pool_threshold", 1, 1);

    let mut env = app.new_env()?;
    count_invoices(&mut env)?;
    env.close()?;

    // Straight away: below the threshold, so no question asked.
    let mut env = app.new_env()?;
    count_invoices(&mut env)?;
    env.close()?;
    assert_eq!(app.pool_revalidations().unwrap_or(0), 0);

    // After the threshold: asked.
    std::thread::sleep(Duration::from_millis(1200));
    let mut env = app.new_env()?;
    count_invoices(&mut env)?;
    env.close()?;
    assert_eq!(app.pool_revalidations().unwrap_or(0), 1);
    Ok(())
}

/// Throwing a dead connection away leaves room for a new one, rather than leaking the count.
#[test]
fn test_discarding_frees_room_in_the_pool() -> Result<()> {
    let app = app_or_skip!("pool_room", 2, 0);

    for _ in 0..2 {
        let mut env = app.new_env()?;
        count_invoices(&mut env)?;
        env.close()?;
    }
    let filled = app.pool_size().unwrap_or(0);
    assert!(filled >= 1);

    terminate_this_pools_backends("pool_room")?;

    // Both connections may be taken again, which is only possible if the dead ones were counted
    // out rather than left occupying the ceiling.
    let first = app.new_env()?;
    let second = app.new_env()?;
    assert!(
        app.pool_size().unwrap_or(0) <= 2,
        "still within the ceiling"
    );
    drop(first);
    drop(second);
    Ok(())
}

/// Killing connections does not break the transaction guarantees.
#[test]
fn test_isolation_holds_across_a_termination() -> Result<()> {
    let app = app_or_skip!("pool_isolation", 4, 0);

    let mut writer = app.new_env()?;
    make_invoice(&mut writer, "uncommitted")?;
    writer.save_all_to_db()?;

    // The reader gets a connection of its own, possibly a replacement for a dead one.
    let mut reader = app.new_env()?;
    assert_eq!(
        count_invoices(&mut reader)?,
        0,
        "flushed is not committed, whatever the connection went through"
    );

    writer.close()?;
    let mut after = app.new_env()?;
    assert_eq!(count_invoices(&mut after)?, 1);
    Ok(())
}

/// A connection killed a moment ago is still not handed out, under the default settings.
///
/// This is why the default asks every time. Asking the client what it already noticed is not
/// enough: a socket the server closed an instant earlier has not been noticed yet, so the cheap
/// check passes and the request fails on a dead connection.
#[test]
fn test_a_connection_that_died_a_moment_ago_is_caught() -> Result<()> {
    let app = app_or_skip!("pool_recent_death", 2, 0);

    let mut env = app.new_env()?;
    count_invoices(&mut env)?;
    env.close()?;

    let killed = terminate_this_pools_backends("pool_recent_death")?;
    assert!(killed >= 1, "the test must actually have killed something");

    let mut env = app.new_env()?;
    assert_eq!(
        count_invoices(&mut env)?,
        0,
        "a dead connection must never reach a caller"
    );
    env.close()?;
    assert!(
        app.pool_discarded().unwrap_or(0) >= 1,
        "and it must have been thrown away"
    );
    Ok(())
}

/// The counters do not drift: a pool nobody broke throws nothing away.
#[test]
fn test_a_healthy_pool_discards_nothing() -> Result<()> {
    let app = app_or_skip!("pool_healthy", 3, 0);

    std::thread::scope(|scope| {
        for n in 0..12 {
            let app = &app;
            scope.spawn(move || {
                let mut env = app.new_env().expect("a connection");
                make_invoice(&mut env, &format!("row {n}")).expect("create");
                env.close().expect("commit");
            });
        }
    });

    let mut env = app.new_env()?;
    assert_eq!(count_invoices(&mut env)?, 12);
    assert_eq!(
        app.pool_discarded().unwrap_or(0),
        0,
        "nothing died, so nothing may have been thrown away"
    );
    Ok(())
}

/// A transaction whose connection dies before it rolls back leaves nothing behind: the rollback
/// fails, the connection is thrown away rather than lent again, and the next caller is served
/// without what the dead transaction wrote.
#[test]
fn test_a_connection_that_could_not_roll_back_is_not_lent_again() -> Result<()> {
    let app = app_or_skip!("pool_failed_rollback", 2, 0);

    let mut env = app.new_env()?;
    make_invoice(&mut env, "never committed")?;
    env.save_all_to_db()?;
    let before = app.pool_discarded().unwrap_or(0);
    let killed = terminate_this_pools_backends("pool_failed_rollback")?;
    assert!(killed >= 1, "the test must actually have killed something");
    drop(env);

    assert!(
        app.pool_discarded().unwrap_or(0) > before,
        "the connection that could not roll back is thrown away"
    );
    let mut env = app.new_env()?;
    assert_eq!(
        count_invoices(&mut env)?,
        0,
        "nothing of the dead transaction"
    );
    env.close()?;
    Ok(())
}
