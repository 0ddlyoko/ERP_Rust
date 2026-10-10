//! The `plugin` table: what the application knows about its plugins, and which are installed.
//!
//! It is what a boot reads to know which plugins to load again, so installing a plugin has to
//! leave a row saying so, and knowing one must never uninstall it.

use base::BasePlugin;
use erp::Result;
use erp::app::Application;
use erp::database::Database;
use erp_search_code_gen::make_domain;
use erp_types::field::MultipleIds;
use erp_types::model::MapOfFields;
use test_utilities::TestLibPlugin;

fn new_app() -> Result<Application> {
    Application::new_test_installed(
        || -> Vec<Box<dyn erp::plugin::Plugin>> {
            vec![Box::new(BasePlugin {}), Box::new(TestLibPlugin {})]
        },
        &["base"],
    )
}

/// The rows naming a plugin, with the fields asked for.
fn rows_of(app: &Application, name: &str, fields: &[&str]) -> Result<Vec<MapOfFields>> {
    let mut env = app.new_env_as_option(None)?;
    let ids = env.search_ids("plugin", &make_domain!([("name", "=", name)]))?;
    env.read("plugin", &MultipleIds::from(ids), fields)
}

#[test]
fn test_installing_a_plugin_records_it() -> Result<()> {
    let app = new_app()?;
    let rows = rows_of(
        &app,
        "base",
        &[
            "state",
            "installed_version",
            "latest_version",
            "description",
        ],
    )?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<&String>("state"), "installed");
    assert_eq!(
        rows[0].get::<&String>("installed_version"),
        env!("CARGO_PKG_VERSION"),
        "base and erp share a version"
    );
    assert_eq!(
        rows[0].get::<&String>("latest_version"),
        rows[0].get::<&String>("installed_version")
    );
    assert!(rows[0].get_option::<&String>("description").is_some());
    Ok(())
}

/// A plugin that is only registered is listed, and not as installed.
#[test]
fn test_a_known_plugin_is_listed_as_not_installed() -> Result<()> {
    let mut app = new_app()?;
    app.record_registered_plugins()?;
    let rows = rows_of(&app, "test_lib_plugin", &["state", "installed_version"])?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<&String>("state"), "not_installed");
    assert!(rows[0].get_option::<&String>("installed_version").is_none());
    Ok(())
}

/// Installing a listed plugin updates its row rather than adding one.
#[test]
fn test_installing_a_listed_plugin_updates_its_row() -> Result<()> {
    let mut app = new_app()?;
    app.record_registered_plugins()?;
    app.load_plugin("test_lib_plugin")?;
    let rows = rows_of(&app, "test_lib_plugin", &["state"])?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<&String>("state"), "installed");
    Ok(())
}

/// A row saying installed keeps saying so when the plugin is only known again, as it is at every
/// boot before the installed plugins are loaded.
#[test]
fn test_knowing_a_plugin_never_uninstalls_it() -> Result<()> {
    let mut app = new_app()?;
    app.record_registered_plugins()?;
    let mut env = app.new_env_as_option(None)?;
    let ids = env.search_ids("plugin", &make_domain!([("name", "=", "test_lib_plugin")]))?;
    let mut values = MapOfFields::default();
    values.insert("state", "installed");
    env.write("plugin", &MultipleIds::from(ids), values)?;
    env.close()?;

    app.record_registered_plugins()?;
    let rows = rows_of(&app, "test_lib_plugin", &["state"])?;
    assert_eq!(rows[0].get::<&String>("state"), "installed");
    Ok(())
}

/// What a boot reads to know which plugins to load again.
#[test]
fn test_the_database_lists_what_to_load_again() -> Result<()> {
    let mut app = new_app()?;
    app.record_registered_plugins()?;
    let mut database = app.create_new_database()?;
    // What a boot does first; the in-memory database lists nothing until it has.
    database.initialize()?;
    assert_eq!(database.get_installed_plugins()?, vec!["base".to_string()]);

    app.load_plugin("test_lib_plugin")?;
    let mut database = app.create_new_database()?;
    let mut installed = database.get_installed_plugins()?;
    installed.sort();
    assert_eq!(
        installed,
        vec!["base".to_string(), "test_lib_plugin".to_string()]
    );
    Ok(())
}
