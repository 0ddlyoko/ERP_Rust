//! Installing a plugin from a call: once the call is answered, the server serves an application
//! with the plugin and what it depends on, and the sessions opened before still hold.

use base::BasePlugin;
use base::models::Users;
use erp::app::Application;
use erp_server::{Server, service};
use serde_json::{Value, json};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use test_utilities::{SeedPlugin, TestLibPlugin};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// The plugins every application of these tests knows: `seed_plugin` depends on
/// `test_lib_plugin`, neither installed until asked.
fn register(app: &mut Application) -> Result<()> {
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(SeedPlugin {}))?;
    Ok(())
}

/// A server on an application with `base` alone, and a token of the administrator's.
async fn start() -> Result<(SocketAddr, Arc<Server>, String)> {
    let mut app = Application::new_test();
    register(&mut app)?;
    app.load_plugin("base")?;
    app.record_registered_plugins()?;
    let token = {
        let mut env = app.new_env()?;
        let authenticated = env.get_empty_record::<Users<_>>().authenticate(
            &mut env,
            "admin".to_string(),
            base::DEFAULT_ADMIN_PASSWORD.to_string(),
        )?;
        env.close()?;
        authenticated.token
    };
    // The plugins are not libraries here, so the next application registers them itself.
    let server = Arc::new(Server::new(app).with_reload(|current, install| {
        let mut next = current.successor();
        register(&mut next)?;
        next.load_plugin("base")?;
        for name in install {
            next.load_plugin(&name)?;
        }
        Ok(next)
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let served = Arc::clone(&server);
    tokio::spawn(async move {
        let _ = axum::serve(listener, service(served)).await;
    });
    Ok((address, server, token))
}

async fn call(address: SocketAddr, token: &str, method: &str, params: Value) -> Result<Value> {
    let body = json!({"jsonrpc": "2.0", "method": method, "params": params, "id": 1});
    let response = reqwest::Client::new()
        .post(format!("http://{address}/jsonrpc"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(body.to_string())
        .send()
        .await?;
    Ok(response.json().await?)
}

/// Pressing Install answers once the plugin and its dependency are installed, and tells the
/// client to load the page again; the application served then declares their models, and the
/// session opened before still holds.
#[tokio::test(flavor = "multi_thread")]
async fn test_installing_a_plugin_serves_it_with_its_dependencies() -> Result<()> {
    let (address, server, token) = start().await?;
    assert!(
        !server
            .application()
            .plugin_manager
            .is_installed("seed_plugin")
    );
    let models = |server: &Server| {
        server
            .application()
            .model_manager
            .try_get_model("sale_order")
            .is_ok()
    };
    assert!(!models(&server), "no such model yet");

    let found = call(
        address,
        &token,
        "plugin.search",
        json!({"domain": [["name", "=", "seed_plugin"]]}),
    )
    .await?;
    let answer = call(
        address,
        &token,
        "plugin.install",
        json!({"ids": found["result"]}),
    )
    .await?;
    assert_eq!(answer["result"], json!({"type": "reload"}), "{answer}");

    let app = server.application();
    assert!(app.plugin_manager.is_installed("seed_plugin"));
    assert!(app.plugin_manager.is_installed("test_lib_plugin"));
    assert!(models(&server));
    let again = call(address, &token, "plugin.search", json!({})).await?;
    assert!(
        again.get("result").is_some(),
        "the session still holds: {again}"
    );
    Ok(())
}
