//! Methods reachable from outside the process.
//!
//! Exposure is asked for every time, never inferred: a method nobody marked does not exist over
//! the wire, and forgetting the mark leaves a missing endpoint rather than an open one.

use erp::app::Application;
use erp_types::field::{IdMode, MultipleIds};
use erp_types::model::MapOfFields;
use serde_json::json;
use std::collections::HashMap;
use std::error::Error;
use test_plugin::TestPlugin;
use test_utilities::TestLibPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app(with_override: bool) -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    if with_override {
        app.register_plugin(Box::new(TestPlugin {}))?;
        app.load_plugin("test_plugin")?;
    } else {
        app.load_plugin("test_lib_plugin")?;
    }
    Ok(app)
}

fn machine(env: &mut erp::environment::Environment, values: &[(&str, i32)]) -> Result<MultipleIds> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    for (field, value) in values {
        map.insert(field, *value);
    }
    env.create_records("machine", vec![map])
}

/// A marked method answers, and its result comes back as JSON.
#[test]
fn test_an_exposed_method_answers() -> Result<()> {
    let app = new_app(false)?;
    let mut env = app.new_env()?;
    let ids = machine(&mut env, &[("base_rate", 100), ("days", 3)])?;

    let out = env.call_rpc("machine", "quote", &json!({"ids": ids.get_ids_ref()}))?;
    assert_eq!(out, json!(300));
    Ok(())
}

/// Arguments arrive named, so adding one later does not break a caller.
#[test]
fn test_arguments_arrive_named() -> Result<()> {
    let app = new_app(false)?;
    let mut env = app.new_env()?;
    let ids = machine(&mut env, &[("base_rate", 50)])?;

    let out = env.call_rpc(
        "machine",
        "quote_for",
        &json!({"ids": ids.get_ids_ref(), "args": {"days": 4}}),
    )?;
    assert_eq!(out, json!(200));
    Ok(())
}

/// A remote call goes through the override chain, like an internal one.
#[test]
fn test_a_remote_call_reaches_the_override() -> Result<()> {
    let app = new_app(true)?;
    let mut env = app.new_env()?;
    let ids = machine(
        &mut env,
        &[("base_rate", 100), ("days", 3), ("discount", 30)],
    )?;

    let out = env.call_rpc("machine", "quote", &json!({"ids": ids.get_ids_ref()}))?;
    assert_eq!(
        out,
        json!(210),
        "three days at the overridden rate of 70, not at the base rate of 100"
    );
    Ok(())
}

/// A method nobody marked is not reachable, however ordinary it looks from Rust.
#[test]
fn test_an_unmarked_method_is_not_reachable() -> Result<()> {
    let app = new_app(false)?;
    let mut env = app.new_env()?;
    let ids = machine(&mut env, &[("base_rate", 100)])?;

    let err = env
        .call_rpc(
            "machine",
            "internal_rate",
            &json!({"ids": ids.get_ids_ref()}),
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot be called remotely"), "got: {err}");
    Ok(())
}

/// A method that does not exist and one that exists but was not marked answer the same way.
#[test]
fn test_absence_and_refusal_are_indistinguishable() -> Result<()> {
    let app = new_app(false)?;
    let mut env = app.new_env()?;
    let ids = machine(&mut env, &[("base_rate", 100)])?;

    let unmarked = env
        .call_rpc(
            "machine",
            "internal_rate",
            &json!({"ids": ids.get_ids_ref()}),
        )
        .unwrap_err()
        .to_string();
    let absent = env
        .call_rpc(
            "machine",
            "no_such_method",
            &json!({"ids": ids.get_ids_ref()}),
        )
        .unwrap_err()
        .to_string();
    assert_eq!(
        unmarked.replace("internal_rate", "X"),
        absent.replace("no_such_method", "X"),
        "what a caller may reach must not tell them what else is there"
    );
    Ok(())
}

/// Arguments that do not fit are refused, not coerced.
#[test]
fn test_bad_arguments_are_refused() -> Result<()> {
    let app = new_app(false)?;
    let mut env = app.new_env()?;
    let ids = machine(&mut env, &[("base_rate", 50)])?;

    assert!(
        env.call_rpc("machine", "quote_for", &json!({"ids": ids.get_ids_ref()}))
            .is_err(),
        "a missing argument"
    );
    assert!(
        env.call_rpc(
            "machine",
            "quote_for",
            &json!({"ids": ids.get_ids_ref(), "args": {"days": "four"}})
        )
        .is_err(),
        "an argument of the wrong type"
    );
    Ok(())
}

/// The registry can say what it holds, and holds only what was marked.
#[test]
fn test_the_registry_lists_only_marked_methods() -> Result<()> {
    let app = new_app(false)?;
    let names = app.model_manager.rpc.names();
    assert!(names.contains(&"machine.quote".to_string()));
    assert!(names.contains(&"machine.quote_for".to_string()));
    assert!(
        !names.iter().any(|name| name.contains("internal_rate")),
        "got: {names:?}"
    );
    Ok(())
}
