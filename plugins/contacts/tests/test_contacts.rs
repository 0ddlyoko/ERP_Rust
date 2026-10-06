//! The address book as an application: its entry in the menu, over what `base` declares.

use base::BasePlugin;
use contacts::ContactsPlugin;
use erp::Result;
use erp::app::Application;
use erp::data;
use mail::MailPlugin;
use serde_json::{Value, json};
use web::WebPlugin;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(WebPlugin {}))?;
    app.register_plugin(Box::new(MailPlugin {}))?;
    app.register_plugin(Box::new(ContactsPlugin {}))?;
    app.load_plugin("contacts")?;
    Ok(app)
}

fn names(entries: &Value) -> Vec<&str> {
    entries
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect()
}

/// Installed, the address book is an application of its own in the menu, opening the contacts,
/// with their tags and the countries to configure.
#[test]
fn test_the_address_book_is_an_application() -> Result<()> {
    let app = new_app()?;
    let admin = {
        let mut env = app.new_env_as_option(None)?;
        data::resolve(&mut env, "base.user_admin")?.expect("seeded")
    };
    let mut env = app.new_env_as_option(Some(admin))?;
    let tree = env.call_rpc("menu", "tree", &json!({}))?;
    let contacts = tree
        .as_array()
        .expect("a list")
        .iter()
        .find(|entry| entry["name"] == "Contacts")
        .expect("the application");
    assert_eq!(
        names(&contacts["children"]),
        vec!["Contacts", "Configuration"]
    );
    assert_eq!(contacts["children"][0]["action"]["model"], "contact");
    assert_eq!(
        names(&contacts["children"][1]["children"]),
        vec!["Tags", "Countries"]
    );
    Ok(())
}

/// A module shows in the colour of the plugin declaring it, and its entries with their icons; an
/// entry under a module has no colour of its own.
#[test]
fn test_a_module_has_its_plugin_colour_and_its_icons() -> Result<()> {
    let app = new_app()?;
    let admin = {
        let mut env = app.new_env_as_option(None)?;
        data::resolve(&mut env, "base.user_admin")?.expect("seeded")
    };
    let mut env = app.new_env_as_option(Some(admin))?;
    let tree = env.call_rpc("menu", "tree", &json!({}))?;
    let find = |name: &str| -> Value {
        tree.as_array()
            .expect("a list")
            .iter()
            .find(|entry| entry["name"] == name)
            .cloned()
            .expect("the module")
    };
    let contacts = find("Contacts");
    assert_eq!(contacts["color"], "#c4501a");
    assert_eq!(contacts["icon"], "contact");
    assert_eq!(contacts["children"][0]["icon"], "users");
    assert_eq!(contacts["children"][0]["color"], Value::Null);
    assert_eq!(find("Settings")["color"], "#5a5878");
    Ok(())
}

/// With the address book installed, the companies are reached from the settings, after the
/// users and groups.
#[test]
fn test_companies_are_in_the_settings() -> Result<()> {
    let app = new_app()?;
    let admin = {
        let mut env = app.new_env_as_option(None)?;
        data::resolve(&mut env, "base.user_admin")?.expect("seeded")
    };
    let mut env = app.new_env_as_option(Some(admin))?;
    let tree = env.call_rpc("menu", "tree", &json!({}))?;
    let settings = tree
        .as_array()
        .expect("a list")
        .iter()
        .find(|entry| entry["name"] == "Settings")
        .expect("the settings");
    let users_companies = &settings["children"][1];
    assert_eq!(
        names(&users_companies["children"]),
        vec!["Users", "Groups", "Companies"]
    );
    assert_eq!(users_companies["children"][2]["action"]["model"], "company");
    Ok(())
}
