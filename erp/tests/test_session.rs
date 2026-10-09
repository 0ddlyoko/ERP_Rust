//! Sessions: earning a token, presenting it, and losing it.
//!
//! The token is `<id>.<secret>`. Only its hash is stored, so it is handed out once and never
//! again, and every way of presenting a bad one — malformed, unknown, revoked, expired, wrong
//! secret — is answered identically. Saying which would tell whoever is guessing how far they got.

use base::models::{Session, Users};
use base::{BasePlugin, DEFAULT_ADMIN_PASSWORD};
use erp::app::Application;
use erp::jsonrpc;
use erp_types::field::{IdMode, MultipleIds, Password, SingleId, TimeDelta, Utc};
use erp_types::model::MapOfFields;
use serde_json::{Value, json};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Result<Application> {
    Application::new_test_installed(
        || -> Vec<Box<dyn erp::plugin::Plugin>> { vec![Box::new(BasePlugin {})] },
        &["base"],
    )
}

fn raw(app: &Application, token: Option<&str>, method: &str, params: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).to_string();
    jsonrpc::handle(app, token, &body).expect("an answer")
}

fn call(app: &Application, token: Option<&str>, method: &str, params: Value) -> Value {
    let answer = raw(app, token, method, params);
    assert!(
        answer.get("error").is_none(),
        "{method} must not fail: {answer}"
    );
    answer["result"].clone()
}

fn make_user(app: &Application, login: &str, password: &str, active: bool) -> Result<u32> {
    let mut env = app.new_env_as_option(None)?;
    let mut values = MapOfFields::default();
    values.insert("login", login);
    values.insert("name", login);
    values.insert("password", Password::new(password)?);
    values.insert("active", active);
    let ids: MultipleIds = env.create_records("users", vec![values])?;
    let uid = ids.get_ids_ref()[0];
    env.close()?;
    Ok(uid)
}

/// Let a user touch sessions through the generic verbs, which no shipped rule grants.
fn grant_sessions(app: &Application, uid: u32) -> Result<()> {
    let mut env = app.new_env_as_option(None)?;
    test_utilities::grant_everything(&mut env, uid, &["session"])?;
    env.close()
}

/// Credentials in, the answer out — `None` when they identify nobody.
///
/// In-process, as the login controller does: authenticating is not reachable over the protocol.
fn try_open(app: &Application, login: &str, password: &str) -> Option<Value> {
    let mut env = app.new_env().expect("an environment");
    let authenticated = env
        .get_empty_record::<Users<_>>()
        .authenticate(&mut env, login.to_string(), password.to_string())
        .ok()?;
    env.close().expect("committed");
    Some(serde_json::to_value(&authenticated).expect("serialisable"))
}

/// Credentials in, token out.
fn open(app: &Application, login: &str, password: &str) -> Value {
    try_open(app, login, password).expect("the credentials identify somebody")
}

/// Who a token speaks for, as the protocol answers it.
fn holder(app: &Application, token: &str) -> u32 {
    call(app, Some(token), "users.me", json!({}))
        .as_u64()
        .expect("somebody") as u32
}

/// End the session `token` opens, as whoever `caller` speaks for.
fn log_out(app: &Application, caller: &str, token: &str) -> Result<bool> {
    let mut env = app.new_env_as(holder(app, caller))?;
    let ended = env
        .get_empty_record::<Users<_>>()
        .log_out(&mut env, token.to_string())?;
    env.close()?;
    Ok(ended)
}

/// The record id the first half of a token names.
fn session_of(token: &str) -> u32 {
    token
        .split_once('.')
        .expect("a token has two halves")
        .0
        .parse()
        .expect("the first half is an id")
}

// ---- earning one ----

#[test]
fn test_the_right_password_opens_a_session() -> Result<()> {
    let app = new_app()?;
    let uid = make_user(&app, "alice", "s3cret", true)?;

    let opened = open(&app, "alice", "s3cret");
    assert_eq!(opened["uid"], json!(uid));
    assert!(opened["token"].as_str().is_some_and(|t| t.contains('.')));
    assert!(opened["expires_at"].as_str().is_some(), "got {opened}");
    Ok(())
}

/// Nothing else does, and each refusal reads the same.
#[test]
fn test_nothing_else_opens_one() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;
    make_user(&app, "retired", "s3cret", false)?;

    for (login, password, what) in [
        ("alice", "wrong", "a wrong password"),
        ("nobody", "s3cret", "an unknown login"),
        ("retired", "s3cret", "an inactive account"),
    ] {
        assert!(
            try_open(&app, login, password).is_none(),
            "{what} opened one"
        );
    }
    Ok(())
}

/// The token is the only copy: what is stored cannot produce it again.
#[test]
fn test_the_token_is_handed_out_once() -> Result<()> {
    let app = new_app()?;
    let uid = make_user(&app, "alice", "s3cret", true)?;
    grant_sessions(&app, uid)?;
    let opened = open(&app, "alice", "s3cret");
    let token = opened["token"].as_str().expect("a token");
    let id = session_of(token);

    let rows = call(
        &app,
        Some(token),
        "session.read",
        json!({"ids": [id], "fields": ["secret", "active"]}),
    );
    assert_eq!(rows[0]["secret"], Value::Null, "got {rows}");
    assert_eq!(rows[0]["active"], json!(true));
    Ok(())
}

/// Two sessions of one account are separate things.
#[test]
fn test_two_sessions_of_one_account_are_independent() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;

    let first = open(&app, "alice", "s3cret");
    let second = open(&app, "alice", "s3cret");
    assert_ne!(first["token"], second["token"]);
    assert_ne!(
        session_of(first["token"].as_str().unwrap()),
        session_of(second["token"].as_str().unwrap())
    );
    Ok(())
}

// ---- presenting one ----

#[test]
fn test_a_token_says_who_is_calling() -> Result<()> {
    let app = new_app()?;
    let uid = make_user(&app, "alice", "s3cret", true)?;
    let opened = open(&app, "alice", "s3cret");
    let token = opened["token"].as_str().expect("a token");

    assert_eq!(call(&app, Some(token), "users.me", json!({})), json!(uid));
    Ok(())
}

/// A caller who authenticated as nobody is still somebody: the portal user.
#[test]
fn test_no_token_is_the_portal_user() -> Result<()> {
    let app = new_app()?;
    let portal = {
        let mut env = app.new_env_as_option(None)?;
        env.named::<Users<SingleId>>("base.user_portal")?.get_id()
    };

    assert_eq!(call(&app, None, "users.me", json!({})), json!(portal));
    Ok(())
}

/// Every bad token is refused the same way, whatever is wrong with it.
#[test]
fn test_every_bad_token_is_refused_alike() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;
    let opened = open(&app, "alice", "s3cret");
    let token = opened["token"].as_str().expect("a token").to_string();
    let (id, secret) = token.split_once('.').expect("two halves");

    let mut refusals = Vec::new();
    for bad in [
        "".to_string(),
        "nonsense".to_string(),
        "not-a-number.".to_string() + secret,
        format!("{}.{secret}", id.parse::<u32>().unwrap() + 1000),
        format!("{id}.wrong"),
        format!("{id}."),
    ] {
        let answer = raw(&app, Some(&bad), "users.me", json!({}));
        assert!(answer.get("result").is_none(), "{bad:?} passed: {answer}");
        refusals.push(answer["error"].clone());
    }

    assert!(
        refusals.windows(2).all(|pair| pair[0] == pair[1]),
        "the refusals differ: {refusals:?}"
    );
    assert_eq!(refusals[0]["code"], json!(-32001));
    Ok(())
}

/// A secret that belongs to another session does not work on this one.
#[test]
fn test_a_secret_is_bound_to_its_own_session() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;
    let first = open(&app, "alice", "s3cret");
    let second = open(&app, "alice", "s3cret");

    let mine = first["token"].as_str().unwrap();
    let theirs = second["token"].as_str().unwrap();
    let crossed = format!("{}.{}", session_of(mine), theirs.split_once('.').unwrap().1);

    let answer = raw(&app, Some(&crossed), "users.me", json!({}));
    assert!(answer.get("result").is_none(), "got {answer}");
    Ok(())
}

// ---- losing one ----

/// Revoking is a write, and the token stops working at once.
#[test]
fn test_revoking_a_session_stops_its_token() -> Result<()> {
    let app = new_app()?;
    let uid = make_user(&app, "alice", "s3cret", true)?;
    grant_sessions(&app, uid)?;
    let opened = open(&app, "alice", "s3cret");
    let token = opened["token"].as_str().expect("a token").to_string();
    assert_eq!(call(&app, Some(&token), "users.me", json!({})), json!(uid));

    call(
        &app,
        Some(&token),
        "session.write",
        json!({"ids": [session_of(&token)], "values": {"active": false}}),
    );

    let answer = raw(&app, Some(&token), "users.me", json!({}));
    assert!(answer.get("result").is_none(), "got {answer}");
    Ok(())
}

/// Logging out ends the session the token opens, and only a session of the caller's own.
#[test]
fn test_logging_out_ends_only_your_own_session() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;
    make_user(&app, "bob", "s3cret", true)?;
    let alice = open(&app, "alice", "s3cret")["token"]
        .as_str()
        .expect("a token")
        .to_string();
    let bob = open(&app, "bob", "s3cret")["token"]
        .as_str()
        .expect("a token")
        .to_string();

    assert!(!log_out(&app, &alice, &bob)?, "not hers to end");
    assert!(
        raw(&app, Some(&bob), "users.me", json!({}))
            .get("result")
            .is_some()
    );

    assert!(log_out(&app, &alice, &alice)?);
    let answer = raw(&app, Some(&alice), "users.me", json!({}));
    assert!(answer.get("result").is_none(), "got {answer}");
    Ok(())
}

/// Opening and ending a session goes through the web client's controllers, not the protocol:
/// neither is reachable over it.
#[test]
fn test_sessions_are_not_handled_over_the_protocol() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;
    let token = open(&app, "alice", "s3cret")["token"]
        .as_str()
        .expect("a token")
        .to_string();
    for (method, params) in [
        (
            "users.authenticate",
            json!({"args": {"login": "alice", "password": "s3cret"}}),
        ),
        ("users.log_out", json!({"args": {"token": token}})),
    ] {
        let answer = raw(&app, Some(&token), method, params);
        assert_eq!(answer["error"]["code"], json!(-32601), "{method}: {answer}");
    }
    Ok(())
}

/// Deleting the record does too — it is still the generic verb, just not the way out.
#[test]
fn test_deleting_a_session_stops_its_token() -> Result<()> {
    let app = new_app()?;
    let uid = make_user(&app, "alice", "s3cret", true)?;
    grant_sessions(&app, uid)?;
    let opened = open(&app, "alice", "s3cret");
    let token = opened["token"].as_str().expect("a token").to_string();

    call(
        &app,
        Some(&token),
        "session.delete",
        json!({"ids": [session_of(&token)]}),
    );

    let answer = raw(&app, Some(&token), "users.me", json!({}));
    assert!(answer.get("result").is_none(), "got {answer}");
    Ok(())
}

/// A session that ran out of time stops on its own.
#[test]
fn test_an_expired_session_stops_its_token() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;
    let opened = open(&app, "alice", "s3cret");
    let token = opened["token"].as_str().expect("a token").to_string();

    {
        let mut env = app.new_env_as_option(None)?;
        let session = Session::<SingleId>::from_id(session_of(&token), &env);
        let past = Utc::now() - TimeDelta::try_seconds(1).expect("a second");
        session.set_expires_at(past, &mut env)?;
        env.close()?;
    }

    let answer = raw(&app, Some(&token), "users.me", json!({}));
    assert!(answer.get("result").is_none(), "got {answer}");
    Ok(())
}

/// It lasts as long as the configuration says.
///
/// Bracketed rather than compared to a single reading of the clock: the call itself takes time —
/// hashing a password is meant to — and how much depends on the machine and what else is running.
#[test]
fn test_a_session_lasts_the_configured_time() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;
    let configured = TimeDelta::try_seconds(i64::try_from(app.server_config().session_duration)?)
        .expect("a length of time");

    let before = Utc::now();
    let opened = open(&app, "alice", "s3cret");
    let after = Utc::now();
    let expires_at: erp_types::field::Timestamp =
        serde_json::from_value(opened["expires_at"].clone())?;

    assert!(
        expires_at >= before + configured && expires_at <= after + configured,
        "expires at {expires_at}, opened between {before} and {after}"
    );
    Ok(())
}

// ---- what a session lets you do ----

/// Changing your own password needs the current one, and retires it.
#[test]
fn test_changing_your_own_password() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;
    let opened = open(&app, "alice", "s3cret");
    let token = opened["token"].as_str().expect("a token").to_string();

    let refused = raw(
        &app,
        Some(&token),
        "users.change_own_password",
        json!({"args": {"current": "wrong", "new": "n3w"}}),
    );
    assert!(refused.get("result").is_none(), "got {refused}");

    let changed = call(
        &app,
        Some(&token),
        "users.change_own_password",
        json!({"args": {"current": "s3cret", "new": "n3w"}}),
    );
    assert_eq!(changed, json!(true));

    assert!(
        try_open(&app, "alice", "s3cret").is_none(),
        "the old password must stop working"
    );
    open(&app, "alice", "n3w");
    Ok(())
}

/// A caller who authenticated as nobody cannot give the portal user a password.
///
/// It is the portal user, so the record it would touch is one that matters: a password there
/// would be a login that anyone could then use. What stops it is the current password, which that
/// account has not got and which no attempt matches.
#[test]
fn test_the_portal_user_cannot_be_given_a_password() -> Result<()> {
    let app = new_app()?;

    for attempt in ["", "portal", "s3cret"] {
        let refused = raw(
            &app,
            None,
            "users.change_own_password",
            json!({"args": {"current": attempt, "new": "n3w"}}),
        );
        assert!(refused.get("result").is_none(), "{attempt:?} got {refused}");
    }
    assert!(try_open(&app, "portal", "n3w").is_none());
    Ok(())
}

/// The seeded administrator can log in with what `post_init` gave it.
#[test]
fn test_the_administrator_can_log_in() -> Result<()> {
    let app = new_app()?;
    let opened = open(&app, "admin", DEFAULT_ADMIN_PASSWORD);

    let token = opened["token"].as_str().expect("a token");
    let uid = call(&app, Some(token), "users.me", json!({}));
    assert_eq!(uid, opened["uid"]);

    let mut env = app.new_env_as_option(None)?;
    let admin = Users::<SingleId>::from_id(opened["uid"].as_u64().unwrap() as u32, &env);
    assert_eq!(admin.get_login(&mut env)?, "admin");
    Ok(())
}

/// Closing an account ends the sessions it already has.
///
/// An inactive account cannot authenticate, and that has to mean the same thing for somebody
/// already holding a token: otherwise closing an account is not a way to lock anybody out.
#[test]
fn test_deactivating_an_account_ends_its_sessions() -> Result<()> {
    let app = new_app()?;
    let uid = make_user(&app, "alice", "s3cret", true)?;
    let opened = open(&app, "alice", "s3cret");
    let token = opened["token"].as_str().expect("a token").to_string();
    assert_eq!(call(&app, Some(&token), "users.me", json!({})), json!(uid));

    {
        let mut env = app.new_env_as_option(None)?;
        let alice = Users::<SingleId>::from_id(uid, &env);
        alice.set_active(false, &mut env)?;
        env.close()?;
    }

    assert!(
        try_open(&app, "alice", "s3cret").is_none(),
        "an inactive account cannot authenticate"
    );

    let answer = raw(&app, Some(&token), "users.me", json!({}));
    assert!(
        answer.get("result").is_none(),
        "nor keep a token it already had: {answer}"
    );
    assert_eq!(answer["error"]["code"], json!(-32001));
    Ok(())
}

// ---- carried by the browser ----

fn browser_call(cookie: &str, csrf: Option<&str>) -> erp::http::Request {
    let request = erp::http::Request::new("POST", "/jsonrpc").with_header("Cookie", cookie);
    match csrf {
        Some(token) => request.with_header("X-CSRF-Token", token),
        None => request,
    }
}

/// A call carried by the session cookie is the session's only with the page's CSRF token; a bearer
/// token or no credential at all needs none.
#[test]
fn test_a_session_cookie_needs_the_pages_csrf_token() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;
    let token = open(&app, "alice", "s3cret")["token"]
        .as_str()
        .expect("a token")
        .to_string();
    let cookie = format!("session_id={token}");
    let csrf = erp::http::csrf::token_for(&app, &browser_call(&cookie, None));

    let code = |answer: std::result::Result<Option<String>, jsonrpc::RpcError>| {
        answer.map_err(|error| error.code)
    };
    let credentials = jsonrpc::credentials(&app, None, &browser_call(&cookie, Some(&csrf)));
    assert_eq!(code(credentials), Ok(Some(token.clone())));

    let refused = jsonrpc::credentials(&app, None, &browser_call(&cookie, None));
    assert_eq!(code(refused), Err(jsonrpc::RpcError::CSRF_REFUSED));
    let elsewhere = erp::http::csrf::token_for(&app, &browser_call("csrf_id=other", None));
    let refused = jsonrpc::credentials(&app, None, &browser_call(&cookie, Some(&elsewhere)));
    assert!(refused.is_err(), "a token of another browser");

    let bearer = jsonrpc::credentials(&app, Some("given"), &browser_call(&cookie, None));
    assert_eq!(
        code(bearer),
        Ok(Some("given".to_string())),
        "the bearer wins, needing no token"
    );
    let nobody = jsonrpc::credentials(&app, None, &browser_call("csrf_id=x", None));
    assert_eq!(code(nobody), Ok(None));
    Ok(())
}

// ---- describing fields ----

/// A private field is not described, even when asked for by name; a caller who may read no
/// record of the model is told nothing about its fields.
#[test]
fn test_fields_get_hides_what_is_private_and_answers_only_readers() -> Result<()> {
    let app = new_app()?;
    let token = open(&app, "admin", DEFAULT_ADMIN_PASSWORD)["token"]
        .as_str()
        .expect("a token")
        .to_string();
    let fields = call(&app, Some(&token), "users.fields_get", json!({}));
    assert!(fields.get("login").is_some(), "{fields}");
    assert!(fields.get("password").is_none(), "{fields}");
    assert_eq!(fields["groups"]["relation_kind"], "many2many");

    let asked = raw(
        &app,
        Some(&token),
        "users.fields_get",
        json!({"fields": ["password"]}),
    );
    assert!(asked.get("result").is_none(), "{asked}");

    let nobody = raw(&app, None, "session.fields_get", json!({}));
    assert!(
        nobody.get("result").is_none(),
        "the portal user reads no session: {nobody}"
    );
    Ok(())
}

/// A name is what a reader of the record sees: the portal user, who may read no account, gets no
/// name for one, rather than a refusal that would fail a whole list.
#[test]
fn test_names_are_only_the_readers() -> Result<()> {
    let app = new_app()?;
    let token = open(&app, "admin", DEFAULT_ADMIN_PASSWORD);
    let admin = token["uid"].clone();
    let token = token["token"].as_str().expect("a token").to_string();

    let named = call(&app, Some(&token), "users.names", json!({"ids": [admin]}));
    assert_eq!(named, json!([[admin, "Administrator"]]));
    let hidden = call(&app, None, "users.names", json!({"ids": [admin]}));
    assert_eq!(hidden, json!([[admin, null]]));

    let uid = admin.as_u64().expect("an id") as u32;
    let mut env = app.new_env_as(uid)?;
    assert_eq!(
        env.names("users", &[uid])?.get(&uid).map(String::as_str),
        Some("Administrator"),
        "the same from Rust, without the protocol"
    );
    let mut nobody = app.new_env()?;
    assert!(nobody.names("users", &[uid])?.is_empty());
    Ok(())
}

/// Choosing a record finds only what the chooser may read.
#[test]
fn test_a_name_search_finds_only_the_readers() -> Result<()> {
    let app = new_app()?;
    let token = open(&app, "admin", DEFAULT_ADMIN_PASSWORD)["token"]
        .as_str()
        .expect("a token")
        .to_string();
    let found = call(
        &app,
        Some(&token),
        "users.name_search",
        json!({"text": "ADMIN"}),
    );
    assert_eq!(found[0][1], "Administrator", "{found}");
    let hidden = call(&app, None, "users.name_search", json!({"text": "admin"}));
    assert_eq!(hidden, json!([]), "the portal user reads no account");
    Ok(())
}
