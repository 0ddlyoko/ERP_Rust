//! Carrying an identity over HTTP.
//!
//! What a token *means* is settled by the protocol tests, which need no server. These check the
//! part only a socket has: the header the token arrives in, and the status a refusal comes back
//! with. The protocol layer knows nothing about statuses — it says "this names nobody" with a code
//! of its own, and the mapping to 401 lives here.

use base::BasePlugin;
use base::models::Users;
use erp::app::Application;
use erp_server::{Server, service};
use serde_json::{Value, json};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// A server, and a token of the administrator's: sessions are opened by the login controller,
/// not over the protocol, so the token is had in-process before the server takes the application.
async fn start() -> Result<(SocketAddr, tokio::task::JoinHandle<()>, String)> {
    let (address, handle, token, _) = start_with_csrf().await?;
    Ok((address, handle, token))
}

/// Same, with the CSRF token a page logged in with that session would hold.
async fn start_with_csrf() -> Result<(SocketAddr, tokio::task::JoinHandle<()>, String, String)> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.load_plugin("base")?;
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
    let page = erp::http::Request::new("GET", "/web")
        .with_header("Cookie", &format!("session_id={token}"));
    let csrf = erp::http::csrf::token_for(&app, &page);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = Arc::new(Server::new(app));
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, service(server)).await;
    });
    Ok((address, handle, token, csrf))
}

struct Answer {
    status: u16,
    challenge: Option<String>,
    body: Option<Value>,
}

async fn post(address: SocketAddr, authorization: Option<&str>, body: Value) -> Result<Answer> {
    send(address, &[("authorization", authorization)], body).await
}

/// A call with these headers, those given as `None` left out.
async fn send(
    address: SocketAddr,
    headers: &[(&str, Option<&str>)],
    body: Value,
) -> Result<Answer> {
    let mut request = reqwest::Client::new()
        .post(format!("http://{address}/jsonrpc"))
        .header("content-type", "application/json");
    for (name, value) in headers {
        if let Some(value) = value {
            request = request.header(*name, *value);
        }
    }
    let response = request.body(body.to_string()).send().await?;
    let status = response.status().as_u16();
    let challenge = response
        .headers()
        .get("www-authenticate")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let text = response.text().await?;
    Ok(Answer {
        status,
        challenge,
        body: serde_json::from_str(&text).ok(),
    })
}

fn request(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params, "id": 1})
}

/// A bearer token arrives in the header and reaches the call.
#[tokio::test]
async fn test_a_bearer_token_identifies_the_caller() -> Result<()> {
    let (address, _server, token) = start().await?;

    let answer = post(
        address,
        Some(&format!("Bearer {token}")),
        request("users.me", json!({})),
    )
    .await?;

    assert_eq!(answer.status, 200);
    let body = answer.body.expect("a body");
    assert!(body["result"].as_u64().is_some(), "got {body}");
    Ok(())
}

/// The scheme is matched without regard to case, as the specification asks.
#[tokio::test]
async fn test_the_scheme_is_case_insensitive() -> Result<()> {
    let (address, _server, token) = start().await?;

    let answer = post(
        address,
        Some(&format!("bearer {token}")),
        request("users.me", json!({})),
    )
    .await?;

    assert_eq!(answer.status, 200);
    assert!(answer.body.expect("a body")["result"].as_u64().is_some());
    Ok(())
}

/// No header at all is not a refusal: it is a caller who authenticated as nobody, which is the
/// portal user rather than nobody at all.
#[tokio::test]
async fn test_no_header_is_answered_normally() -> Result<()> {
    let (address, _server, _) = start().await?;

    let answer = post(address, None, request("users.me", json!({}))).await?;

    assert_eq!(answer.status, 200);
    assert!(
        answer.body.expect("a body")["result"].as_u64().is_some(),
        "the portal user is somebody"
    );
    Ok(())
}

/// A token that names nobody comes back as the status HTTP has for exactly that, with the
/// challenge the specification requires alongside it.
#[tokio::test]
async fn test_a_token_naming_nobody_is_a_401() -> Result<()> {
    let (address, _server, _) = start().await?;

    let answer = post(
        address,
        Some("Bearer 404.nonsense"),
        request("users.me", json!({})),
    )
    .await?;

    assert_eq!(answer.status, 401);
    assert_eq!(answer.challenge.as_deref(), Some("Bearer"));
    let body = answer.body.expect("a body");
    assert_eq!(body["error"]["code"], json!(-32001), "got {body}");
    Ok(())
}

/// A header in another scheme carries no token, so the call runs as nobody rather than being
/// refused: there is nothing there to refuse.
#[tokio::test]
async fn test_another_scheme_is_not_a_token() -> Result<()> {
    let (address, _server, _) = start().await?;

    let answer = post(
        address,
        Some("Basic YWRtaW46YWRtaW4="),
        request("users.me", json!({})),
    )
    .await?;

    assert_eq!(answer.status, 200);
    assert!(answer.body.expect("a body")["result"].as_u64().is_some());
    Ok(())
}

/// A revoked token stops being accepted by the server, not only by the protocol layer.
#[tokio::test]
async fn test_a_revoked_token_is_refused_over_http() -> Result<()> {
    let (address, _server, token) = start().await?;
    let id: u32 = token.split_once('.').expect("two halves").0.parse()?;
    let bearer = format!("Bearer {token}");

    let answer = post(
        address,
        Some(&bearer),
        request(
            "session.write",
            json!({"ids": [id], "values": {"active": false}}),
        ),
    )
    .await?;
    assert_eq!(answer.status, 200);

    let answer = post(address, Some(&bearer), request("users.me", json!({}))).await?;
    assert_eq!(answer.status, 401);
    Ok(())
}

/// A page of the web client calls with the session cookie and the CSRF token it was given; the
/// cookie alone, which any site can make the browser send, is refused.
#[tokio::test]
async fn test_the_session_cookie_carries_a_call_with_its_csrf_token() -> Result<()> {
    let (address, _server, token, csrf) = start_with_csrf().await?;
    let cookie = format!("session_id={token}");

    let answer = send(
        address,
        &[("cookie", Some(&cookie)), ("x-csrf-token", Some(&csrf))],
        request("users.me", json!({})),
    )
    .await?;
    assert_eq!(answer.status, 200);
    let body = answer.body.expect("a body");
    assert!(body["result"].as_u64().is_some(), "got {body}");

    let answer = send(
        address,
        &[("cookie", Some(&cookie))],
        request("users.me", json!({})),
    )
    .await?;
    assert_eq!(answer.status, 400);
    let body = answer.body.expect("a body");
    assert_eq!(
        body["error"]["code"],
        json!(erp::jsonrpc::RpcError::CSRF_REFUSED)
    );
    Ok(())
}
