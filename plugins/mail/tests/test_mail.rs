//! The thread of a record: changes of its tracked fields, read by whoever reads the record.

use base::BasePlugin;
use erp::app::Application;
use erp::data;
use erp::types::field::{IdMode, MultipleIds};
use mail::MailPlugin;
use serde_json::{Value, json};
use std::error::Error;
use web::WebPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(WebPlugin {}))?;
    app.register_plugin(Box::new(MailPlugin {}))?;
    app.load_plugin("mail")?;
    Ok(app)
}

fn resolve(app: &Application, xml_id: &str) -> Result<u32> {
    let mut env = app.new_env_as_option(None)?;
    Ok(data::resolve(&mut env, xml_id)?.expect("seeded"))
}

/// Write as the administrator, in a unit of work of its own.
fn write(app: &Application, model: &str, id: u32, values: Value) -> Result<()> {
    let admin = resolve(app, "base.user_admin")?;
    let mut env = app.new_env_as_option(Some(admin))?;
    env.call_rpc(model, "write", &json!({"ids": [id], "values": values}))?;
    env.close()
}

fn thread(app: &Application, model: &str, id: u32) -> Result<Value> {
    let admin = resolve(app, "base.user_admin")?;
    let mut env = app.new_env_as_option(Some(admin))?;
    env.call_rpc(
        "message",
        "thread",
        &json!({"ids": [], "args": {"model": model, "record": id}}),
    )
}

/// Changing tracked fields notes them, by whom, as a person reads them; other fields, and a
/// field written back to what it held, are not noted.
#[test]
fn test_changes_of_tracked_fields_are_noted() -> Result<()> {
    let app = new_app()?;
    let portal = resolve(&app, "base.user_portal")?;
    assert_eq!(thread(&app, "users", portal)?, json!([]));

    write(
        &app,
        "users",
        portal,
        json!({"login": "visitor", "active": true, "name": "Guest"}),
    )?;
    write(&app, "users", portal, json!({"name": "Guest again"}))?;
    write(&app, "users", portal, json!({"login": "visitor"}))?;

    let messages = thread(&app, "users", portal)?;
    let messages = messages.as_array().expect("a thread");
    assert_eq!(messages.len(), 1, "{messages:?}");
    let message = &messages[0];
    assert_eq!(message["kind"], "tracking");
    assert_eq!(message["author"][1], "Administrator");
    let mut changes: Vec<Value> = message["changes"].as_array().expect("changes").clone();
    changes.sort_by_key(|change| change["field"].as_str().unwrap_or_default().to_string());
    assert_eq!(
        changes,
        [
            json!({"field": "active", "label": "Active", "old": "No", "new": "Yes",
                   "old_value": "false", "new_value": "true"}),
            json!({"field": "login", "label": "Login", "old": "portal", "new": "visitor",
                   "old_value": "portal", "new_value": "visitor"}),
        ]
    );
    Ok(())
}

/// A thread is read by whoever may read its record, and nobody else.
#[test]
fn test_a_thread_is_read_with_its_record() -> Result<()> {
    let app = new_app()?;
    let admin = resolve(&app, "base.user_admin")?;
    let portal = resolve(&app, "base.user_portal")?;
    write(&app, "users", admin, json!({"login": "boss"}))?;
    let mut env = app.new_env_as_option(Some(portal))?;
    let refused = env.call_rpc(
        "message",
        "thread",
        &json!({"ids": [], "args": {"model": "users", "record": admin}}),
    );
    assert!(refused.is_err(), "a portal user may not read users");
    Ok(())
}

/// Deleting a record deletes its thread.
#[test]
fn test_a_deleted_record_takes_its_thread_along() -> Result<()> {
    let app = new_app()?;
    let portal = resolve(&app, "base.user_portal")?;
    write(&app, "users", portal, json!({"login": "visitor"}))?;
    let mut env = app.new_env_as_option(None)?;
    let of_users = erp_search_code_gen::make_domain!([("model", "=", "users")]);
    let messages = env.search_ids("message", &of_users)?;
    assert_eq!(messages.len(), 1);
    let changes_before = env
        .search_ids("message_change", &erp::search::SearchType::Nothing)?
        .len();
    env.delete("users", &MultipleIds::from(vec![portal]))?;
    assert!(env.search_ids("message", &of_users)?.is_empty());
    assert_eq!(
        env.search_ids("message_change", &erp::search::SearchType::Nothing)?
            .len(),
        changes_before - 1,
        "its one change went with it"
    );
    env.close()
}

/// Over JSON-RPC, the changes are saved once the call is over, no longer as the caller: they are
/// still noted as the caller's.
#[test]
fn test_changes_are_noted_as_the_caller_over_json_rpc() -> Result<()> {
    let app = new_app()?;
    let portal = resolve(&app, "base.user_portal")?;
    let token = {
        let mut env = app.new_env()?;
        let authenticated = env
            .get_empty_record::<base::models::Users<_>>()
            .authenticate(
                &mut env,
                "admin".to_string(),
                base::DEFAULT_ADMIN_PASSWORD.to_string(),
            )?;
        env.close()?;
        authenticated.token
    };
    let body = json!({
        "jsonrpc": "2.0",
        "method": "users.write",
        "params": {"ids": [portal], "values": {"login": "visitor"}},
        "id": 1
    });
    let answer = erp::jsonrpc::handle(&app, Some(&token), &body.to_string()).expect("owed");
    assert_eq!(answer["result"], true, "{answer}");
    assert_eq!(
        thread(&app, "users", portal)?[0]["author"][1],
        "Administrator"
    );
    Ok(())
}

mod tracked {
    use code_gen::Model;
    use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};

    #[derive(Model)]
    #[erp(id = "project")]
    #[allow(dead_code)]
    pub struct Project<Mode: IdMode> {
        pub id: Mode,
        #[erp(default = "")]
        name: String,
        #[erp(inverse = "project", tracking)]
        tasks: Reference<BaseTask, MultipleIds>,
        #[erp(relation = "project_watcher_rel", tracking)]
        watchers: Reference<BaseTask, MultipleIds>,
    }

    #[derive(Model)]
    #[erp(id = "task")]
    #[allow(dead_code)]
    pub struct Task<Mode: IdMode> {
        pub id: Mode,
        #[erp(default = "")]
        name: String,
        project: Reference<BaseProject, SingleId>,
        #[erp(relation = "project_watcher_rel")]
        watched: Reference<BaseProject, MultipleIds>,
    }
}

struct TrackedPlugin;

impl erp::plugin::Plugin for TrackedPlugin {
    fn name(&self) -> String {
        "tracked".to_string()
    }

    fn init_models(&self, model_manager: &mut erp::model::ModelManager) {
        model_manager.register_model::<tracked::Project<_>>();
        model_manager.register_model::<tracked::Task<_>>();
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["mail".to_string()]
    }
}

/// A one2many and a many2many are tracked like any field: the names of the records they held,
/// and of those they hold. Holding the same records in another order is no change.
#[test]
fn test_lists_of_records_are_tracked() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(WebPlugin {}))?;
    app.register_plugin(Box::new(MailPlugin {}))?;
    app.register_plugin(Box::new(TrackedPlugin {}))?;
    app.load_plugin("tracked")?;

    let mut env = app.new_env_as_option(None)?;
    let project = env
        .create_records("project", vec![named("Moon")])?
        .get_ids_ref()[0];
    let tasks = env.create_records("task", vec![named("Design"), named("Build"), named("Test")])?;
    let tasks = tasks.get_ids_ref().to_vec();
    env.call_rpc(
        "project",
        "write",
        &json!({"ids": [project], "values": {"tasks": [tasks[0], tasks[1]], "watchers": [tasks[2]]}}),
    )?;
    env.close()?;

    let mut env = app.new_env_as_option(None)?;
    env.call_rpc(
        "project",
        "write",
        &json!({"ids": [project], "values": {"tasks": [tasks[1], tasks[0]], "watchers": [tasks[2]]}}),
    )?;
    env.close()?;

    let mut env = app.new_env_as_option(None)?;
    env.call_rpc(
        "project",
        "write",
        &json!({"ids": [project], "values": {"tasks": [tasks[1]], "watchers": []}}),
    )?;
    env.close()?;

    let mut env = app.new_env_as_option(None)?;
    let thread = env.call_rpc(
        "message",
        "thread",
        &json!({"ids": [], "args": {"model": "project", "record": project}}),
    )?;
    let changes: Vec<Value> = thread
        .as_array()
        .expect("a thread")
        .iter()
        .map(|message| {
            let mut changes = message["changes"].as_array().expect("changes").clone();
            changes.sort_by_key(|change| change["field"].as_str().unwrap_or_default().to_string());
            Value::Array(changes)
        })
        .collect();
    assert_eq!(
        changes,
        [
            json!([
                {"field": "tasks", "label": "Tasks", "old": "Design, Build", "new": "Build",
                 "old_value": "1,2", "new_value": "2"},
                {"field": "watchers", "label": "Watchers", "old": "Test", "new": null,
                 "old_value": "3", "new_value": null},
            ]),
            json!([
                {"field": "tasks", "label": "Tasks", "old": null, "new": "Design, Build",
                 "old_value": null, "new_value": "1,2"},
                {"field": "watchers", "label": "Watchers", "old": null, "new": "Test",
                 "old_value": null, "new_value": "3"},
            ]),
            json!([]),
        ],
        "newest first; the reordering noted nothing; created with nothing in either"
    );
    Ok(())
}

fn named(name: &str) -> erp::types::model::MapOfFields {
    let mut values = erp::types::model::MapOfFields::default();
    values.insert("name", name);
    values
}

/// A record whose model tracks fields starts its thread with its creation and what those fields
/// started as; a thread asked for one field holds only the messages noting it.
#[test]
fn test_creation_starts_the_thread_and_a_field_narrows_it() -> Result<()> {
    let app = new_app()?;
    let admin = resolve(&app, "base.user_admin")?;
    let mut env = app.new_env_as_option(Some(admin))?;
    let created = env.call_rpc(
        "users",
        "create",
        &json!({"values": {"name": "Claire", "login": "claire", "groups": [1]}}),
    )?;
    env.close()?;
    let claire = created[0].as_u64().expect("an id") as u32;
    write(&app, "users", claire, json!({"login": "claire.m"}))?;

    let messages = thread(&app, "users", claire)?;
    let creation = &messages[1];
    assert_eq!(creation["kind"], "creation");
    assert_eq!(creation["author"][1], "Administrator");
    let mut fields: Vec<&str> = creation["changes"]
        .as_array()
        .expect("changes")
        .iter()
        .map(|change| change["field"].as_str().unwrap_or_default())
        .collect();
    fields.sort();
    assert_eq!(
        fields,
        ["active", "groups", "login"],
        "what they started as"
    );
    assert_eq!(
        messages.as_array().map_or(0, Vec::len),
        2,
        "created with them, not changed"
    );

    let narrowed = |field: &str| -> Result<usize> {
        let mut env = app.new_env_as_option(Some(admin))?;
        let found = env.call_rpc(
            "message",
            "thread",
            &json!({"ids": [], "args": {"model": "users", "record": claire, "field": field}}),
        )?;
        Ok(found.as_array().map_or(0, Vec::len))
    };
    assert_eq!(narrowed("login")?, 2, "created, then changed");
    assert_eq!(narrowed("active")?, 1, "created only");
    assert_eq!(narrowed("name")?, 0, "not tracked");
    Ok(())
}
