//! The fields `base` computes: a menu's full name, the number of users of a group — saved, and
//! worked out by an onchange as the form changes.

use base::BasePlugin;
use erp::app::Application;
use erp::data;
use serde_json::{Value, json};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.load_plugin("base")?;
    Ok(app)
}

/// One call as the administrator, in its own unit of work, as a request is answered.
fn call(app: &Application, method: &str, params: Value) -> Value {
    let admin = {
        let mut env = app.new_env_as_option(None).expect("an environment");
        data::resolve(&mut env, "base.user_admin")
            .expect("resolved")
            .expect("seeded")
    };
    let mut env = app.new_env_as_option(Some(admin)).expect("an environment");
    let (model, operation) = method.rsplit_once('.').expect("model.operation");
    let answer = env
        .call_rpc(model, operation, &params)
        .unwrap_or_else(|error| panic!("{method} failed: {error}"));
    env.close().expect("committed");
    answer
}

/// A menu's full name follows its parents, and its own name as the form changes it.
#[test]
fn test_a_menu_full_name() -> Result<()> {
    let app = new_app()?;
    let parent = call(&app, "menu.create", json!({"values": {"name": "Sales"}}))[0].clone();
    let child = call(
        &app,
        "menu.create",
        json!({"values": {"name": "Orders", "parent": parent}}),
    )[0]
    .clone();
    let read = call(
        &app,
        "menu.read",
        json!({"ids": [child], "fields": ["complete_name"]}),
    );
    assert_eq!(read[0]["complete_name"], "Sales / Orders");

    let answer = call(
        &app,
        "menu.onchange",
        json!({"id": child, "values": {"name": "Quotations"}}),
    );
    assert_eq!(answer["values"]["complete_name"], "Sales / Quotations");
    Ok(())
}

/// A group counts its users, and so does its form as users are taken out.
#[test]
fn test_a_group_user_count() -> Result<()> {
    let app = new_app()?;
    let user = call(
        &app,
        "users.create",
        json!({"values": {"login": "ann", "name": "Ann"}}),
    )[0]
    .clone();
    let group = call(
        &app,
        "group.create",
        json!({"values": {"name": "Sales", "users": [user]}}),
    )[0]
    .clone();
    let read = call(
        &app,
        "group.read",
        json!({"ids": [group], "fields": ["user_count"]}),
    );
    assert_eq!(read[0]["user_count"], 1);

    let answer = call(
        &app,
        "group.onchange",
        json!({"id": group, "values": {"users": {"unlink": [user]}}}),
    );
    assert_eq!(answer["values"]["user_count"], 0);
    Ok(())
}

/// A user created without a contact gets one, named as they are; one given a contact keeps it. A
/// contact a user is cannot be deleted.
#[test]
fn test_a_user_gets_a_contact() -> Result<()> {
    let app = new_app()?;
    let user = call(
        &app,
        "users.create",
        json!({"values": {"login": "ann@example.com", "name": "Ann"}}),
    )[0]
    .clone();
    let read = call(
        &app,
        "users.read",
        json!({"ids": [user], "fields": ["contact"]}),
    );
    let contact = read[0]["contact"].clone();
    let contact_read = call(
        &app,
        "contact.read",
        json!({"ids": [contact], "fields": ["name"]}),
    );
    assert_eq!(contact_read[0]["name"], "Ann");

    let given = call(&app, "contact.create", json!({"values": {"name": "Bob's"}}))[0].clone();
    let bob = call(
        &app,
        "users.create",
        json!({"values": {"login": "bob", "name": "Bob", "contact": given}}),
    )[0]
    .clone();
    let read = call(
        &app,
        "users.read",
        json!({"ids": [bob], "fields": ["contact"]}),
    );
    assert_eq!(read[0]["contact"], given);

    let admin = {
        let mut env = app.new_env_as_option(None)?;
        data::resolve(&mut env, "base.user_admin")?.expect("seeded")
    };
    let mut env = app.new_env_as_option(Some(admin))?;
    let refused = env.call_rpc("contact", "delete", &json!({"ids": [contact]}));
    assert!(refused.is_err(), "Ann's contact is hers");
    Ok(())
}

/// The users `base` seeds have their contact too.
#[test]
fn test_seeded_users_have_a_contact() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let admin = data::resolve(&mut env, "base.user_admin")?.expect("seeded");
    let rows = env.read(
        "users",
        &erp_types::field::SingleId::from(admin),
        &["contact"],
    )?;
    assert!(rows[0].get_option::<&u32>("contact").is_some());
    Ok(())
}
