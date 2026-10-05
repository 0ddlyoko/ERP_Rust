//! The contacts `base` declares: named after their company, their address on lines, and the
//! companies their company field offers.

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

fn create(app: &Application, values: Value) -> Value {
    call(app, "contact.create", json!({ "values": values }))[0].clone()
}

fn read(app: &Application, id: &Value, field: &str) -> Value {
    call(app, "contact.read", json!({"ids": [id], "fields": [field]}))[0][field].clone()
}

/// A person working for a company is named after it, and follows it when it is renamed; a
/// company, or a person on their own, is named as they are.
#[test]
fn test_a_person_is_named_after_their_company() -> Result<()> {
    let app = new_app()?;
    let acme = create(&app, json!({"name": "Acme", "is_company": true}));
    let john = create(&app, json!({"name": "John Doe", "parent": acme}));
    assert_eq!(read(&app, &acme, "complete_name"), "Acme");
    assert_eq!(read(&app, &john, "complete_name"), "Acme, John Doe");

    call(
        &app,
        "contact.write",
        json!({"ids": [acme], "values": {"name": "Acme Inc."}}),
    );
    assert_eq!(read(&app, &john, "complete_name"), "Acme Inc., John Doe");

    call(
        &app,
        "contact.write",
        json!({"ids": [john], "values": {"parent": null}}),
    );
    assert_eq!(read(&app, &john, "complete_name"), "John Doe");
    Ok(())
}

/// Searching a person by name finds them by their company's as well, and shows them so.
#[test]
fn test_a_person_is_found_by_their_company() -> Result<()> {
    let app = new_app()?;
    let acme = create(&app, json!({"name": "Acme", "is_company": true}));
    create(&app, json!({"name": "John Doe", "parent": acme}));
    let found = call(&app, "contact.name_search", json!({"text": "acme"}));
    let names: Vec<&str> = found
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|pair| pair[1].as_str())
        .collect();
    assert_eq!(names, vec!["Acme", "Acme, John Doe"]);
    Ok(())
}

/// The address is the lines filled in, the country by its name; none when nothing is.
#[test]
fn test_the_address_is_the_lines_filled_in() -> Result<()> {
    let app = new_app()?;
    let belgium = {
        let mut env = app.new_env_as_option(None)?;
        data::resolve(&mut env, "base.country_be")?.expect("seeded")
    };
    let office = create(
        &app,
        json!({
            "name": "Office",
            "street": "Rue de la Loi 16",
            "zip": "1000",
            "city": "Brussels",
            "country": belgium,
        }),
    );
    assert_eq!(
        read(&app, &office, "address"),
        "Rue de la Loi 16\n1000 Brussels\nBelgium"
    );
    let nowhere = create(&app, json!({"name": "Nowhere"}));
    assert_eq!(read(&app, &nowhere, "address"), Value::Null);
    Ok(())
}

/// The company field offers companies only.
#[test]
fn test_the_company_field_offers_companies() -> Result<()> {
    let app = new_app()?;
    let fields = call(&app, "contact.fields_get", json!({"fields": ["parent"]}));
    assert_eq!(
        fields["parent"]["domain"],
        json!([["is_company", "=", true]])
    );
    assert_eq!(fields["parent"]["label"], "Company");
    Ok(())
}

/// Archiving a contact keeps it, inactive, until it is brought back.
#[test]
fn test_contacts_archive() -> Result<()> {
    let app = new_app()?;
    let someone = create(&app, json!({"name": "Someone"}));
    assert_eq!(read(&app, &someone, "active"), true);
    call(&app, "contact.archive", json!({"ids": [someone]}));
    assert_eq!(read(&app, &someone, "active"), false);
    call(&app, "contact.unarchive", json!({"ids": [someone]}));
    assert_eq!(read(&app, &someone, "active"), true);
    Ok(())
}
