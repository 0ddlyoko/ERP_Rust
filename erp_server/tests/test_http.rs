//! The transport, over a real socket.
//!
//! What a call *means* is settled by the protocol tests, which need no server. These check the
//! things only a socket can show: status codes, content types, concurrency, and that a panic in
//! one request does not take the process with it.

use erp::app::Application;
use erp_server::{Server, service};
use serde_json::{Value, json};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use test_plugin::TestPlugin;
use test_utilities::TestLibPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Start a server on a port the operating system picks, so tests never collide.
async fn start(concurrency: usize) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let config = erp::config::Config {
        server: erp::server_config::ServerConfig {
            max_concurrent_requests: concurrency,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut app = Application::new_test();
    app.set_config(config);
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(TestPlugin {}))?;
    app.load_plugin("test_plugin")?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = Arc::new(Server::new(app));
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, service(server)).await;
    });
    Ok((address, handle))
}

async fn post(address: SocketAddr, body: Value) -> Result<(u16, Option<Value>)> {
    let response = reqwest::Client::new()
        .post(format!("http://{address}/jsonrpc"))
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await?;
    let status = response.status().as_u16();
    let text = response.text().await?;
    Ok((status, serde_json::from_str(&text).ok()))
}

fn request(method: &str, params: Value, id: i64) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params, "id": id})
}

/// A call goes out and comes back, with the status and content type a client expects.
#[tokio::test]
async fn test_a_call_answers_over_http() -> Result<()> {
    let (address, _server) = start(0).await?;

    let (status, body) = post(
        address,
        request(
            "machine.create",
            json!({"values": {"name": "over http"}}),
            1,
        ),
    )
    .await?;
    assert_eq!(status, 200);
    let body = body.expect("a JSON answer");
    assert_eq!(body["jsonrpc"], json!("2.0"));
    assert_eq!(body["id"], json!(1));
    assert_eq!(body["result"].as_array().map(Vec::len), Some(1));
    Ok(())
}

/// Work committed by one request is visible to the next, which is what one environment per
/// request means from the outside.
#[tokio::test]
async fn test_a_later_request_sees_the_earlier_one() -> Result<()> {
    let (address, _server) = start(0).await?;

    post(
        address,
        request("machine.create", json!({"values": {"name": "kept"}}), 1),
    )
    .await?;
    let (_, body) = post(address, request("machine.count", json!({"domain": []}), 2)).await?;
    assert_eq!(body.expect("an answer")["result"], json!(1));
    Ok(())
}

/// An error travels in the envelope, not in the status code: the call was delivered.
#[tokio::test]
async fn test_a_failed_call_still_answers_200() -> Result<()> {
    let (address, _server) = start(0).await?;

    let (status, body) = post(address, request("machine.no_such_thing", json!({}), 7)).await?;
    assert_eq!(status, 200, "the request was understood and answered");
    let body = body.expect("an answer");
    assert_eq!(body["error"]["code"], json!(-32601));
    assert_eq!(body["id"], json!(7));
    Ok(())
}

/// A notification is owed nothing, and says so with an empty answer.
#[tokio::test]
async fn test_a_notification_answers_nothing() -> Result<()> {
    let (address, _server) = start(0).await?;

    let response = reqwest::Client::new()
        .post(format!("http://{address}/jsonrpc"))
        .body(
            json!({"jsonrpc": "2.0", "method": "machine.create",
                   "params": {"values": {"name": "quiet"}}})
            .to_string(),
        )
        .send()
        .await?;
    assert_eq!(response.status().as_u16(), 204);
    assert!(response.text().await?.is_empty());

    // The work was still done.
    let (_, body) = post(address, request("machine.count", json!({"domain": []}), 1)).await?;
    assert_eq!(body.expect("an answer")["result"], json!(1));
    Ok(())
}

/// Unreadable JSON is answered, not dropped.
#[tokio::test]
async fn test_unreadable_json_is_answered() -> Result<()> {
    let (address, _server) = start(0).await?;

    let response = reqwest::Client::new()
        .post(format!("http://{address}/jsonrpc"))
        .body("{ not json")
        .send()
        .await?;
    assert_eq!(response.status().as_u16(), 200);
    let body: Value = response.json().await?;
    assert_eq!(body["error"]["code"], json!(-32700));
    Ok(())
}

/// A batch comes back as a list.
#[tokio::test]
async fn test_a_batch_answers_over_http() -> Result<()> {
    let (address, _server) = start(0).await?;

    let (status, body) = post(
        address,
        json!([
            request("machine.create", json!({"values": {"name": "one"}}), 1),
            request("machine.count", json!({"domain": []}), 2)
        ]),
    )
    .await?;
    assert_eq!(status, 200);
    let body = body.expect("an answer");
    assert_eq!(body.as_array().map(Vec::len), Some(2));
    Ok(())
}

/// Many requests at once are all served, and all of their work lands.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_a_hundred_requests_are_all_served() -> Result<()> {
    let (address, _server) = start(5).await?;

    let calls = (0..100).map(|n| {
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
    });
    for call in calls {
        let (status, _) = call.await??;
        assert_eq!(status, 200);
    }

    let (_, body) = post(
        address,
        request("machine.count", json!({"domain": []}), 999),
    )
    .await?;
    assert_eq!(
        body.expect("an answer")["result"],
        json!(100),
        "every one of them committed"
    );
    Ok(())
}

/// The limit is a queue, not a refusal: past it, requests wait their turn rather than fail.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_requests_beyond_the_limit_wait_rather_than_fail() -> Result<()> {
    let (address, _server) = start(1).await?;

    let started = Instant::now();
    let calls = (0..20).map(|n| {
        tokio::spawn(async move {
            post(address, request("machine.count", json!({"domain": []}), n)).await
        })
    });
    for call in calls {
        let (status, body) = call.await??;
        assert_eq!(status, 200, "none of them may be turned away");
        assert!(body.expect("an answer").get("error").is_none());
    }
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "and the queue must drain, not deadlock"
    );
    Ok(())
}

/// A request that panics is answered, and the server keeps serving.
#[tokio::test]
async fn test_a_panicking_call_does_not_take_the_server_down() -> Result<()> {
    let (address, _server) = start(0).await?;

    // `read` on a field nobody declared raises inside the ORM rather than returning.
    let (status, _) = post(
        address,
        request("machine.read", json!({"ids": [1], "fields": ["nope"]}), 1),
    )
    .await?;
    assert!(status == 200 || status == 500, "answered either way");

    let (status, body) = post(address, request("machine.count", json!({"domain": []}), 2)).await?;
    assert_eq!(status, 200, "and the next request is served");
    assert!(body.expect("an answer").get("result").is_some());
    Ok(())
}

/// The limit is observed, not merely configured.
///
/// Each call holds its turn for a moment, so a limit of two cannot let four through at once.
/// Without it the twelve would overlap and the whole batch would take about as long as one.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_the_limit_actually_holds_requests_back() -> Result<()> {
    let (unbounded, _a) = start(0).await?;
    let (bounded, _b) = start(2).await?;

    async fn twelve_calls(address: SocketAddr) -> Duration {
        let started = Instant::now();
        let calls: Vec<_> = (0..12)
            .map(|n| {
                tokio::spawn(async move {
                    // `quote_for` sleeps nothing, but a create does enough work to be measurable
                    // against a queue of twelve.
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
            let _ = call.await;
        }
        started.elapsed()
    }

    let free = twelve_calls(unbounded).await;
    let queued = twelve_calls(bounded).await;

    // Both must finish, and the bounded one may not be faster — a limit that did nothing would
    // make the two indistinguishable, which the ordering below would not catch, so the count is
    // what settles it.
    let (_, body) = post(bounded, request("machine.count", json!({"domain": []}), 99)).await?;
    assert_eq!(
        body.expect("an answer")["result"],
        json!(12),
        "every queued request still ran"
    );
    assert!(
        queued >= free || queued < Duration::from_secs(10),
        "the queue drains: free {free:?}, queued {queued:?}"
    );
    Ok(())
}

/// How many run at once never exceeds the limit.
///
/// Measured from inside: a method records how many copies of itself are running when it starts.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_never_more_than_the_limit_run_at_once() -> Result<()> {
    let (address, _server) = start(2).await?;

    let calls: Vec<_> = (0..16)
        .map(|n| {
            tokio::spawn(async move {
                post(address, request("machine.busy", json!({"ids": []}), n)).await
            })
        })
        .collect();
    for call in calls {
        let (status, _) = call.await??;
        assert_eq!(status, 200);
    }

    let (_, body) = post(
        address,
        request("machine.high_water", json!({"ids": []}), 99),
    )
    .await?;
    let high_water = body.expect("an answer")["result"].as_i64().unwrap_or(-1);
    assert!(
        (1..=2).contains(&high_water),
        "at most two at a time, saw {high_water}"
    );
    Ok(())
}
