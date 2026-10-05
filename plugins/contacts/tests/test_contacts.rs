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
