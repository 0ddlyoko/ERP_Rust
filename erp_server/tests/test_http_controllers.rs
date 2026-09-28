//! Controllers answer over HTTP, not only through `erp::http::handle`.

use base::BasePlugin;
use erp::app::Application;
use erp_server::{Server, service};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

async fn start() -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.load_plugin("base")?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = Arc::new(Server::new(app));
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, service(server)).await;
    });
    Ok((address, handle))
}

#[tokio::test]
async fn test_the_root_is_served() -> Result<()> {
    let (address, _server) = start().await?;
    let response = reqwest::get(format!("http://{address}/")).await?;
    assert_eq!(response.status().as_u16(), 200);
    assert!(
        response
            .headers()
            .get("content-type")
            .and_then(|kind| kind.to_str().ok())
            .is_some_and(|kind| kind.starts_with("text/html"))
    );
    assert!(response.text().await?.contains("running"));
    Ok(())
}

#[tokio::test]
async fn test_refusals_keep_their_status_and_headers() -> Result<()> {
    let (address, _server) = start().await?;
    let response = reqwest::get(format!("http://{address}/nowhere")).await?;
    assert_eq!(response.status().as_u16(), 404);

    let response = reqwest::Client::new()
        .delete(format!("http://{address}/"))
        .send()
        .await?;
    assert_eq!(response.status().as_u16(), 405);
    assert_eq!(
        response
            .headers()
            .get("allow")
            .and_then(|v| v.to_str().ok()),
        Some("GET")
    );
    Ok(())
}

/// The protocol keeps its own route.
#[tokio::test]
async fn test_jsonrpc_is_still_answered() -> Result<()> {
    let (address, _server) = start().await?;
    let response = reqwest::Client::new()
        .post(format!("http://{address}/jsonrpc"))
        .header("content-type", "application/json")
        .body(r#"{"jsonrpc":"2.0","method":"users.me","params":{},"id":1}"#)
        .send()
        .await?;
    assert_eq!(response.status().as_u16(), 200);
    Ok(())
}
