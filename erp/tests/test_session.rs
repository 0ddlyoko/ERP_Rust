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
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.load_plugin("base")?;
    Ok(app)
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
    let mut env = app.new_env()?;
    let mut values = MapOfFields::default();
    values.insert("login", login);
    values.insert("password", Password::new(password)?);
    values.insert("active", active);
    let ids: MultipleIds = env.create_records("users", vec![values])?;
    let uid = ids.get_ids_ref()[0];
    env.close()?;
    Ok(uid)
}

/// Credentials in, token out.
fn open(app: &Application, login: &str, password: &str) -> Value {
    call(
        app,
        None,
        "users.authenticate",
        json!({"args": {"login": login, "password": password}}),
    )
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
        let answer = raw(
            &app,
            None,
            "users.authenticate",
            json!({"args": {"login": login, "password": password}}),
        );
        assert!(
            answer.get("result").is_none(),
            "{what} opened one: {answer}"
        );
    }
    Ok(())
}

/// The token is the only copy: what is stored cannot produce it again.
#[test]
fn test_the_token_is_handed_out_once() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;
    let opened = open(&app, "alice", "s3cret");
    let id = session_of(opened["token"].as_str().expect("a token"));

    let rows = call(
        &app,
        None,
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
        let mut env = app.new_env()?;
        erp::data::resolve(&mut env, "base.user_portal")?.expect("a seeded portal user")
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

/// Deleting the record does too — it is still the generic verb, just not the way out.
#[test]
fn test_deleting_a_session_stops_its_token() -> Result<()> {
    let app = new_app()?;
    make_user(&app, "alice", "s3cret", true)?;
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
        let mut env = app.new_env()?;
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

    call(
        &app,
        Some(&token),
        "users.change_own_password",
        json!({"args": {"current": "s3cret", "new": "n3w"}}),
    );

    assert!(
        raw(
            &app,
            None,
            "users.authenticate",
            json!({"args": {"login": "alice", "password": "s3cret"}})
        )
        .get("result")
        .is_none(),
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

    let still_refused = raw(
        &app,
        None,
        "users.authenticate",
        json!({"args": {"login": "portal", "password": "n3w"}}),
    );
    assert!(still_refused.get("result").is_none(), "got {still_refused}");
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

    let mut env = app.new_env()?;
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
        let mut env = app.new_env()?;
        let alice = Users::<SingleId>::from_id(uid, &env);
        alice.set_active(false, &mut env)?;
        env.close()?;
    }

    assert!(
        raw(
            &app,
            None,
            "users.authenticate",
            json!({"args": {"login": "alice", "password": "s3cret"}})
        )
        .get("result")
        .is_none(),
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
