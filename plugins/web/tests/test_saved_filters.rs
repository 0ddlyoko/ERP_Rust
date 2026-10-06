//! Searches users save on a list, to find them again: each user's own, one opening the list.

use base::BasePlugin;
use erp::Result;
use erp::app::Application;
use erp::data;
use erp::environment::Environment;
use erp::serde_json::{Value, json};
use erp::types::field::IdMode;
use erp::types::model::MapOfFields;
use web::WebPlugin;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(WebPlugin {}))?;
    app.load_plugin("web")?;
    Ok(app)
}

fn call(env: &mut Environment, method: &str, args: Value) -> Result<Value> {
    env.call_rpc("saved_filter", method, &json!({"ids": [], "args": args}))
}

const ORDERS: &str = "sale.action_orders";

/// What a user saves comes back to them on the same list, by name; the list opens with the one
/// saved as such, and only one of theirs.
#[test]
fn test_a_user_finds_their_searches_again() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    let admin = data::resolve(&mut env, "base.user_admin")?.expect("an administrator");
    let mut env = env.as_user(admin);
    let late = json!([{"kind": "filters", "names": ["late"]}]);
    call(
        &mut env,
        "save",
        json!({"action": ORDERS, "name": "Late", "facets": late, "is_default": true}),
    )?;
    call(
        &mut env,
        "save",
        json!({"action": ORDERS, "name": "Big", "facets": [], "is_default": true}),
    )?;
    call(
        &mut env,
        "save",
        json!({"action": "other", "name": "Elsewhere", "facets": [], "is_default": false}),
    )?;
    let mine = call(&mut env, "mine", json!({"action": ORDERS}))?;
    let names: Vec<(&str, bool)> = mine
        .as_array()
        .expect("searches")
        .iter()
        .map(|saved| {
            (
                saved["name"].as_str().unwrap_or_default(),
                saved["is_default"] == true,
            )
        })
        .collect();
    assert_eq!(names, vec![("Big", true), ("Late", false)]);
    assert_eq!(mine[1]["facets"], late);
    let error = call(
        &mut env,
        "save",
        json!({"action": ORDERS, "name": " ", "facets": [], "is_default": false}),
    )
    .expect_err("no name")
    .to_string();
    assert!(error.contains("has a name"), "{error}");
    Ok(())
}

/// Another user neither sees nor forgets them, nor reads the model directly.
#[test]
fn test_searches_stay_their_users() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    let admin = data::resolve(&mut env, "base.user_admin")?.expect("an administrator");
    let mut values = MapOfFields::default();
    values.insert("login", "clerk");
    values.insert("name", "Clerk");
    let clerk = env.sudo().create_records("users", vec![values])?.get_ids_ref()[0];
    let saved = call(
        &mut env.as_user(admin),
        "save",
        json!({"action": ORDERS, "name": "Mine", "facets": [], "is_default": false}),
    )?;
    let mut env = env.as_user(clerk);
    assert_eq!(
        call(&mut env, "mine", json!({"action": ORDERS}))?,
        json!([])
    );
    let error = call(&mut env, "forget", json!({"id": saved}))
        .expect_err("not theirs")
        .to_string();
    assert!(error.contains("not one of yours"), "{error}");
    assert!(env.call_rpc("saved_filter", "search", &json!({})).is_err());
    let mut env = env.as_user(admin);
    assert_eq!(call(&mut env, "forget", json!({"id": saved}))?, json!(true));
    assert_eq!(
        call(&mut env, "mine", json!({"action": ORDERS}))?,
        json!([])
    );
    Ok(())
}
