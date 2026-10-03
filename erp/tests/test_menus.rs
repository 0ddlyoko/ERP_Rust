//! Actions and menus: what a client offers to open, and to whom.

use base::models::Menu;
use base::{BasePlugin, DEFAULT_ADMIN_PASSWORD};
use erp::app::Application;
use erp::data;
use erp::model::ModelManager;
use erp::plugin::Plugin;
use erp_types::field::{IdMode, MultipleIds, Password};
use erp_types::model::MapOfFields;
use serde_json::{Value, json};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// A plugin shipping data files only, menus among them.
struct DataPlugin {
    data: &'static [&'static str],
}

impl Plugin for DataPlugin {
    fn name(&self) -> String {
        "menu_plugin".to_string()
    }

    fn init_models(&self, _model_manager: &mut ModelManager) {}

    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string()]
    }

    fn data(&self) -> Vec<&'static str> {
        self.data.to_vec()
    }
}

fn new_app(data: &'static [&'static str]) -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(DataPlugin { data }))?;
    app.load_plugin("menu_plugin")?;
    Ok(app)
}

fn admin(app: &Application) -> Result<u32> {
    let mut env = app.new_env_as_option(None)?;
    Ok(data::resolve(&mut env, "base.user_admin")?.expect("seeded"))
}

/// A user in the users' group only, not an administrator.
fn employee(app: &Application) -> Result<u32> {
    let mut env = app.new_env_as_option(None)?;
    let users_group = data::resolve(&mut env, "base.group_user")?.expect("seeded");
    let mut values = MapOfFields::default();
    values.insert("login", "employee");
    values.insert("password", Password::new("s3cret")?);
    values.insert(
        "groups",
        erp_types::field::FieldType::Refs(vec![users_group]),
    );
    let ids: MultipleIds = env.create_records("users", vec![values])?;
    env.close()?;
    Ok(ids.get_ids_ref()[0])
}

/// Base's menus, as an administrator sees them: a module, groups, sections and their entries.
fn settings() -> Value {
    json!([[
        "Settings",
        [
            ["Users & Companies", [["Users", []], ["Groups", []]]],
            [
                "Technical",
                [
                    [
                        "User interface",
                        [["Views", []], ["Menus", []], ["Actions", []]]
                    ],
                    ["Security", [["Access rules", []]]],
                    ["Plugins", []]
                ]
            ]
        ]
    ]])
}

fn tree(app: &Application, uid: Option<u32>) -> Result<Value> {
    let mut env = app.new_env_as_option(uid.or(app.model_manager.identities.default_user()))?;
    env.get_empty_record::<Menu<_>>().tree(&mut env)
}

/// The names of a tree, nested as it is.
fn names(tree: &Value) -> Value {
    Value::Array(
        tree.as_array()
            .expect("entries")
            .iter()
            .map(|entry| json!([entry["name"], names(&entry["children"])]))
            .collect(),
    )
}

#[test]
fn test_the_administrator_sees_the_settings() -> Result<()> {
    let app = new_app(&[])?;
    let tree = tree(&app, Some(admin(&app)?))?;
    assert_eq!(names(&tree), settings());
    assert_eq!(tree[0]["action"], Value::Null, "a title");
    let users = &tree[0]["children"][0]["children"][0]["action"];
    assert_eq!(
        users,
        &json!({
            "id": users["id"],
            "xml_id": "base.action_users",
            "name": "Users",
            "model": "users",
            "views": ["list", "form"],
            "domain": [],
        })
    );
    Ok(())
}

/// A menu of a group is shown to its members only; nobody logged in sees none of base's.
#[test]
fn test_menus_are_shown_to_their_groups() -> Result<()> {
    let app = new_app(&[])?;
    assert_eq!(tree(&app, Some(employee(&app)?))?, json!([]));
    assert_eq!(tree(&app, None)?, json!([]));
    Ok(())
}

const MENUS: &[&str] = &[r#"<erp>
    <action id="action_groups" name="Groups" model="group" views="list"
            domain='[["name", "!=", ""]]'/>
    <menu id="menu_people" name="People">
        <menu id="menu_later" name="Later" sequence="20" action="action_groups"/>
        <menu id="menu_first" name="First" sequence="5" action="action_groups"/>
        <menu id="menu_admins" name="Admins only" action="action_groups" groups="base.group_admin"/>
    </menu>
    <menu id="menu_empty" name="Nothing under it"/>
</erp>"#];

/// Entries come by sequence; a title leading nowhere is left out; an action's domain is read.
#[test]
fn test_a_tree_is_ordered_and_leads_somewhere() -> Result<()> {
    let app = new_app(MENUS)?;
    let tree = tree(&app, Some(employee(&app)?))?;
    assert_eq!(
        names(&tree),
        json!([["People", [["First", []], ["Later", []]]]])
    );
    assert_eq!(
        tree[0]["children"][0]["action"]["domain"],
        json!([["name", "!=", ""]])
    );
    assert_eq!(tree[0]["children"][0]["action"]["views"], json!(["list"]));
    Ok(())
}

/// A client asks for its menus over the protocol.
#[test]
fn test_the_tree_is_reached_over_the_protocol() -> Result<()> {
    let app = new_app(&[])?;
    let token = {
        let mut env = app.new_env()?;
        let authenticated = env
            .get_empty_record::<base::models::Users<_>>()
            .authenticate(
                &mut env,
                "admin".to_string(),
                DEFAULT_ADMIN_PASSWORD.to_string(),
            )?;
        env.close()?;
        authenticated.token
    };
    let body = json!({"jsonrpc": "2.0", "method": "menu.tree", "params": {"ids": [], "args": {}}, "id": 1});
    let answer = erp::jsonrpc::handle(&app, Some(&token), &body.to_string()).expect("owed");
    assert_eq!(names(&answer["result"]), settings(), "{answer}");
    Ok(())
}

/// Menus, actions and views are read by clients through `menu.tree` and `view.load`, which show
/// each user what is theirs; reading them otherwise is for administrators.
#[test]
fn test_only_administrators_read_menus_actions_and_views_directly() -> Result<()> {
    let app = new_app(&[])?;
    let employee = employee(&app)?;
    for model in ["menu", "action", "view"] {
        let mut env = app.new_env_as(employee)?;
        let refused = env.search_ids(model, &erp_search::SearchType::Nothing);
        assert!(refused.is_err(), "{model} is closed to an employee");

        let mut env = app.new_env_as(admin(&app)?)?;
        assert!(
            !env.search_ids(model, &erp_search::SearchType::Nothing)?
                .is_empty(),
            "{model}"
        );
    }

    let mut env = app.new_env_as(employee)?;
    let list = env.get_empty_record::<base::models::View<_>>().load(
        &mut env,
        "users".to_string(),
        "list".to_string(),
    )?;
    assert!(
        list.starts_with("<list>"),
        "views still reach the client: {list}"
    );
    assert_eq!(
        tree(&app, Some(employee))?,
        json!([]),
        "and so do its menus"
    );
    Ok(())
}

/// A button may open any action by its external identifier, whoever its form is shown to.
#[test]
fn test_an_action_is_loaded_by_its_identifier() -> Result<()> {
    let app = new_app(&[])?;
    let mut env = app.new_env_as(employee(&app)?)?;
    let action = env
        .get_empty_record::<base::models::Action<_>>()
        .load(&mut env, "base.action_users".to_string())?;
    assert_eq!(action["model"], "users");
    assert_eq!(action["views"], json!(["list", "form"]));
    let missing = env
        .get_empty_record::<base::models::Action<_>>()
        .load(&mut env, "base.nowhere".to_string());
    assert!(missing.is_err());
    Ok(())
}
