//! The company running the database: seeded, found by the code that needs it, shown in Settings.

use base::BasePlugin;
use base::models::{Company, Contact, View};
use erp::Result;
use erp::app::Application;
use erp::data;
use erp::environment::Environment;
use erp::types::field::SingleId;
use erp::types::model::MapOfFields;
use serde_json::json;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.load_plugin("base")?;
    Ok(app)
}

fn admin_env(app: &Application) -> Result<Environment<'_>> {
    let admin = {
        let mut env = app.new_env_as_option(None)?;
        data::resolve(&mut env, "base.user_admin")?.expect("seeded")
    };
    app.new_env_as_option(Some(admin))
}

/// A fresh database has one company, a company contact holding its address, and it is the
/// current one.
#[test]
fn test_a_main_company_is_seeded_and_current() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let main: Company<SingleId> = env.named("base.main_company")?;
    assert_eq!(main.get_name(&mut env)?, "My Company");
    let contact: Contact<SingleId> = main.get_contact(&mut env)?;
    assert!(*contact.get_is_company(&mut env)?);
    assert_eq!(contact.get_name(&mut env)?, "My Company");

    let current = Company::current(&mut env)?;
    assert_eq!(current.get_id(), main.get_id());
    Ok(())
}

/// The oldest company stays the current one when another is added.
#[test]
fn test_the_current_company_is_the_first() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let main: Company<SingleId> = env.named("base.main_company")?;
    let contact = env.call_rpc(
        "contact",
        "create",
        &json!({"values": {"name": "Branch", "is_company": true}}),
    )?;
    let contact = contact[0].as_u64().expect("an id") as u32;
    let mut values = MapOfFields::default();
    values.insert("name", "Branch");
    values.insert("contact", contact);
    env.create_records("company", vec![values])?;
    assert_eq!(Company::current(&mut env)?.get_id(), main.get_id());
    Ok(())
}

/// A company needs its contact, and that contact cannot be deleted while the company exists.
#[test]
fn test_a_company_keeps_its_contact() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let mut values = MapOfFields::default();
    values.insert("name", "No address");
    let error = env
        .create_records("company", vec![values])
        .expect_err("a company needs a contact")
        .to_string();
    assert!(error.contains("contact"), "{error}");

    let main: Company<SingleId> = env.named("base.main_company")?;
    let contact: Contact<SingleId> = main.get_contact(&mut env)?;
    assert!(
        env.delete("contact", &SingleId::from(contact.get_id()))
            .is_err(),
        "the company's contact is restricted"
    );
    Ok(())
}

/// Companies have a form showing their contact's address, and an action opening them.
#[test]
fn test_companies_have_their_views() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let form = env.get_empty_record::<View<_>>().load(
        &mut env,
        "company".to_string(),
        "form".to_string(),
    )?;
    assert!(
        form.contains(r#"<field name="contact" widget="contact""#),
        "{form}"
    );
    let action = data::resolve(&mut env, "base.action_companies")?;
    assert!(action.is_some(), "the action is declared");
    Ok(())
}
