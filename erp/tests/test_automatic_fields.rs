//! The fields the ORM fills in on every model: when, and by whom, a record was created and last
//! changed.

use base::BasePlugin;
use base::models::Group;
use erp::app::Application;
use erp::data;
use erp_types::field::SingleId;
use erp_types::model::MapOfFields;
use serde_json::json;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Result<(Application, u32)> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.load_plugin("base")?;
    let admin = {
        let mut env = app.new_env_as_option(None)?;
        data::resolve(&mut env, "base.user_admin")?.expect("seeded")
    };
    Ok((app, admin))
}

/// Creating fills in both dates and both users; writing moves only the last change.
#[test]
fn test_creating_and_writing_are_noted() -> Result<()> {
    let (app, admin) = new_app()?;
    let mut env = app.new_env_as_option(Some(admin))?;
    let mut values = MapOfFields::default();
    values.insert("name", "Sales");
    let group: Group<SingleId> = env.create_new_record_from_map(values)?;
    let created = *group.get_create_date(&mut env)?.expect("noted");
    assert_eq!(group.get_create_uid(&mut env)?, Some(admin));
    assert_eq!(group.get_write_uid(&mut env)?, Some(admin));
    assert_eq!(group.get_write_date(&mut env)?.copied(), Some(created));
    env.close()?;

    std::thread::sleep(std::time::Duration::from_millis(5));
    let mut env = app.new_env_as_option(None)?;
    let mut values = MapOfFields::default();
    values.insert("name", "Sales team");
    env.write("group", &group.id, values)?;
    env.close()?;

    let mut env = app.new_env_as_option(None)?;
    assert_eq!(
        *group.get_create_date(&mut env)?.expect("kept"),
        created,
        "creation stays"
    );
    assert_eq!(group.get_create_uid(&mut env)?, Some(admin));
    assert!(
        *group.get_write_date(&mut env)?.expect("noted") > created,
        "the change is later"
    );
    assert_eq!(
        group.get_write_uid(&mut env)?,
        None,
        "nobody in particular changed it"
    );
    Ok(())
}

/// Nobody writes them but the ORM, and a client is told they are read only.
#[test]
fn test_nobody_else_writes_them() -> Result<()> {
    let (app, admin) = new_app()?;
    let mut env = app.new_env_as_option(Some(admin))?;
    let described = env.call_rpc(
        "group",
        "fields_get",
        &json!({"fields": ["create_date", "create_uid"]}),
    )?;
    assert_eq!(described["create_date"]["readonly"], true);
    assert_eq!(described["create_uid"]["relation"], "users");

    let created = env.call_rpc(
        "group",
        "create",
        &json!({"values": {"name": "x", "create_date": "2000-01-01T00:00:00Z"}}),
    );
    assert!(
        created
            .expect_err("refused")
            .to_string()
            .contains("filled in by the ORM"),
        "a creation date is not given"
    );
    let ids = env.call_rpc("group", "create", &json!({"values": {"name": "y"}}))?;
    let refused = env.call_rpc(
        "group",
        "write",
        &json!({"ids": ids, "values": {"write_uid": admin}}),
    );
    assert!(
        refused
            .expect_err("refused")
            .to_string()
            .contains("filled in by the ORM")
    );
    Ok(())
}
