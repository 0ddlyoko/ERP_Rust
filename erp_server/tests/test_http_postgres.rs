//! The server against a real database.
//!
//! The rest of the HTTP tests run on the in-memory backend, which is why none of them noticed
//! that the PostgreSQL driver is synchronous: it builds a runtime of its own and blocks on it,
//! so reaching it from a thread already driving one panics. Only a test that speaks to a real
//! server can see that.
//!
//! Skipped, not failed, when none answers.

use erp::app::Application;
use erp::config::Config;
use erp::database::{DatabaseConfig, DatabaseType};
use erp_server::{Server, service};
use serde_json::{Value, json};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use test_utilities::TestLibPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// A directory with no plugin in it, shared by every test here.
fn empty_plugin_directory() -> String {
    let path = std::env::temp_dir().join("erp_http_pg_plugins");
    std::fs::create_dir_all(&path).expect("a directory to scan");
    path.to_string_lossy().into_owned()
}

fn config(schema: &str) -> Config {
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
        // An empty directory rather than an empty path: loading scans it, and a path that is
        // not there is an error rather than an absence of plugins.
        plugin_path: empty_plugin_directory(),
        server: erp::server_config::ServerConfig {
            max_concurrent_requests: 3,
            ..Default::default()
        },
    }
}

/// Build an application against a schema wiped clean, ready to be loaded.
fn prepare(schema: &str) -> Option<Application> {
    let mut app = Application::new(config(schema));
    // Registered by hand rather than found on disk: the directory is deliberately empty, so
    // these tests never depend on what happens to be built there.
    app.register_plugin(Box::new(base::BasePlugin {})).ok()?;
    app.register_plugin(Box::new(TestLibPlugin {})).ok()?;

    let mut database = app.create_new_database().ok()?;
    let DatabaseType::Postgres(connection) = &mut database else {
        return None;
    };
    connection
        .client
        .batch_execute(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
        .expect("dropping the schema");
    drop(database);

    Some(app)
}

/// Every test runs on a runtime, as a request does. Loading goes through the crate's own
/// helper, which is what keeps it off the runtime's threads — if it ever stopped doing that,
/// these tests would panic exactly as the binary did.
macro_rules! app_or_skip {
    ($schema:expr) => {
        // Preparing talks to the server too, so it is no more allowed on the runtime than
        // loading is.
        match tokio::task::spawn_blocking(|| prepare($schema)).await? {
            Some(app) => {
                let app = erp_server::load(app)
                    .await
                    .unwrap_or_else(|error| panic!("loading against PostgreSQL: {error}"));
                // A plugin is loaded because it is installed, not because it is registered, and
                // this schema was just wiped. Talking to the server, so off the runtime.
                tokio::task::spawn_blocking(move || {
                    let mut app = app;
                    app.load_plugin("test_lib_plugin")
                        .expect("installing the plugin");
                    // Every request here comes in anonymous, and nothing shipped grants the
                    // portal user anything on a test model.
                    let portal = app
                        .model_manager
                        .identities
                        .default_user()
                        .expect("base names the anonymous caller");
                    let mut env = app.new_env_as_option(None).expect("an environment");
                    test_utilities::grant_everything(&mut env, portal, &["machine"])
                        .expect("granting the anonymous caller");
                    env.close().expect("committing the grant");
                    app
                })
                .await?
            }
            None => {
                eprintln!("skipping: no PostgreSQL server reachable");
                return Ok(());
            }
        }
    };
}

async fn start(app: Application) -> Result<SocketAddr> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = Arc::new(Server::new(app));
    tokio::spawn(async move {
        let _ = axum::serve(listener, service(server)).await;
    });
    Ok(address)
}

async fn post(address: SocketAddr, body: Value) -> Result<Value> {
    let text = reqwest::Client::new()
        .post(format!("http://{address}/jsonrpc"))
        .body(body.to_string())
        .send()
        .await?
        .text()
        .await?;
    Ok(serde_json::from_str(&text)?)
}

fn request(method: &str, params: Value, id: i64) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params, "id": id})
}

/// A call travels from the socket to PostgreSQL and back.
///
/// This is the whole point: the driver is reached from a runtime thread, which is exactly what
/// used to panic.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_a_call_reaches_postgres_and_returns() -> Result<()> {
    let app = app_or_skip!("http_pg_basic");
    let address = start(app).await?;

    let created = post(
        address,
        request(
            "machine.create",
            json!({"values": {"name": "over http"}}),
            1,
        ),
    )
    .await?;
    assert!(
        created.get("error").is_none(),
        "the call must not fail: {created}"
    );
    assert_eq!(created["result"].as_array().map(Vec::len), Some(1));

    let counted = post(address, request("machine.count", json!({"domain": []}), 2)).await?;
    assert_eq!(counted["result"], json!(1), "and it must have committed");
    Ok(())
}

/// Each request gets its own transaction, against the real server.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_a_failed_call_rolls_back_against_postgres() -> Result<()> {
    let app = app_or_skip!("http_pg_rollback");
    let address = start(app).await?;

    let refused = post(
        address,
        request(
            "machine.create",
            json!({"values": {"name": "half", "base_rate": "not a number"}}),
            1,
        ),
    )
    .await?;
    assert!(refused.get("error").is_some());

    let counted = post(address, request("machine.count", json!({"domain": []}), 2)).await?;
    assert_eq!(counted["result"], json!(0), "nothing may have survived");
    Ok(())
}

/// Many requests at once, all reaching a pool smaller than their number.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_many_requests_share_the_pool() -> Result<()> {
    let app = app_or_skip!("http_pg_pool");
    let address = start(app).await?;

    let calls: Vec<_> = (0..24)
        .map(|n| {
            tokio::spawn(async move {
                post(
                    address,
                    request(
                        "machine.create",
                        json!({"values": {"name": format!("n{n}")}}),
                        n,
                    ),
                )
                .await
            })
        })
        .collect();
    for call in calls {
        let answer = call.await??;
        assert!(answer.get("error").is_none(), "got {answer}");
    }

    let counted = post(address, request("machine.count", json!({"domain": []}), 99)).await?;
    assert_eq!(
        counted["result"],
        json!(24),
        "a pool of four served twenty-four callers"
    );
    Ok(())
}

/// Searching and reading travel too, not only writing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_searching_and_reading_over_http_against_postgres() -> Result<()> {
    let app = app_or_skip!("http_pg_read");
    let address = start(app).await?;

    for rate in [10, 20, 30] {
        post(
            address,
            request(
                "machine.create",
                json!({"values": {"name": format!("m{rate}"), "base_rate": rate}}),
                rate,
            ),
        )
        .await?;
    }

    let rows = post(
        address,
        request(
            "machine.read_matching",
            json!({"domain": [["base_rate", ">=", 20]], "fields": ["base_rate"],
                   "order": ["base_rate desc"]}),
            1,
        ),
    )
    .await?;
    let rates: Vec<i64> = rows["result"]
        .as_array()
        .expect("a list of rows")
        .iter()
        .map(|row| row["base_rate"].as_i64().unwrap_or(-1))
        .collect();
    assert_eq!(rates, vec![30, 20]);
    Ok(())
}
