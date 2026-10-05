//! When a plugin's data files are loaded again.
//!
//! On install, when the plugin's version changes, and when an update is asked for — never on an
//! ordinary boot, which would otherwise undo whatever users changed in records a file declared.

use base::BasePlugin;
use erp::app::{Application, DataUpdate, LaunchArgs};
use erp::data;
use erp::database::cache::CacheDatabase;
use erp_search_code_gen::make_domain;
use erp_types::field::{MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::error::Error;
use test_utilities::{SeedPlugin, TestLibPlugin};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Start the application on this database, as a restart of the server would.
fn boot(database: &CacheDatabase, update: DataUpdate) -> Result<Application> {
    let mut app = Application::new_test();
    app.cache_db = database.clone();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(SeedPlugin {}))?;
    app.set_data_update(update);
    app.load_plugin("seed_plugin")?;
    Ok(app)
}

/// Rename the order the seed file declares, the way a user would.
fn rename_seeded_order(app: &Application, name: &str) -> Result<()> {
    let mut env = app.new_env_as_option(None)?;
    let order = data::resolve(&mut env, "seed_plugin.main_order")?.expect("seeded");
    let mut values = MapOfFields::default();
    values.insert("name", name);
    env.write("sale_order", &SingleId::from(order), values)?;
    env.close()
}

fn seeded_order_name(app: &Application) -> Result<String> {
    let mut env = app.new_env_as_option(None)?;
    let order = data::resolve(&mut env, "seed_plugin.main_order")?.expect("seeded");
    let rows = env.read("sale_order", &SingleId::from(order), &["name"])?;
    Ok(rows[0].get::<&String>("name").clone())
}

#[test]
fn test_a_boot_with_the_same_version_leaves_data_alone() -> Result<()> {
    let database = CacheDatabase::default();
    let app = boot(&database, DataUpdate::Nothing)?;
    rename_seeded_order(&app, "Renamed by a user")?;
    drop(app);

    let app = boot(&database, DataUpdate::Nothing)?;
    assert_eq!(seeded_order_name(&app)?, "Renamed by a user");
    Ok(())
}

#[test]
fn test_a_new_version_loads_data_again() -> Result<()> {
    let database = CacheDatabase::default();
    let app = boot(&database, DataUpdate::Nothing)?;
    rename_seeded_order(&app, "Renamed by a user")?;
    let mut env = app.new_env_as_option(None)?;
    let row = env.search_ids("plugin", &make_domain!([("name", "=", "seed_plugin")]))?;
    let mut values = MapOfFields::default();
    values.insert("installed_version", "0.0.0");
    env.write("plugin", &MultipleIds::from(row), values)?;
    env.close()?;
    drop(app);

    let app = boot(&database, DataUpdate::Nothing)?;
    assert_eq!(seeded_order_name(&app)?, "Seeded order");
    Ok(())
}

#[test]
fn test_asking_for_an_update_loads_data_again() -> Result<()> {
    for update in [
        DataUpdate::Only(vec!["seed_plugin".to_string()]),
        DataUpdate::All,
    ] {
        let database = CacheDatabase::default();
        let app = boot(&database, DataUpdate::Nothing)?;
        rename_seeded_order(&app, "Renamed by a user")?;
        drop(app);

        let app = boot(&database, update.clone())?;
        assert_eq!(seeded_order_name(&app)?, "Seeded order", "with {update:?}");
    }
    Ok(())
}

/// Updating one plugin leaves the others' data alone.
#[test]
fn test_an_update_only_reaches_the_plugins_named() -> Result<()> {
    let database = CacheDatabase::default();
    let app = boot(&database, DataUpdate::Nothing)?;
    rename_seeded_order(&app, "Renamed by a user")?;
    drop(app);

    let app = boot(&database, DataUpdate::Only(vec!["base".to_string()]))?;
    assert_eq!(seeded_order_name(&app)?, "Renamed by a user");
    Ok(())
}

#[test]
fn test_the_command_line_says_what_to_update() -> Result<()> {
    let parse = |args: &[&str]| {
        LaunchArgs::from_args(args.iter().map(|arg| arg.to_string())).map(|launch| launch.update)
    };
    assert_eq!(parse(&[])?, DataUpdate::Nothing);
    assert_eq!(parse(&["-u", "all"])?, DataUpdate::All);
    assert_eq!(
        parse(&["--update=base, seed_plugin"])?,
        DataUpdate::Only(vec!["base".to_string(), "seed_plugin".to_string()])
    );
    assert_eq!(
        parse(&["-u", "base", "--update", "seed_plugin"])?,
        DataUpdate::Only(vec!["base".to_string(), "seed_plugin".to_string()])
    );
    assert_eq!(parse(&["-u", "base", "-u", "all"])?, DataUpdate::All);
    assert!(parse(&["-u"]).is_err(), "a flag without a value");
    assert!(parse(&["--updat", "base"]).is_err(), "a mistyped flag");
    Ok(())
}

#[test]
fn test_the_command_line_says_what_to_install() -> Result<()> {
    let parse = |args: &[&str]| LaunchArgs::from_args(args.iter().map(|arg| arg.to_string()));
    let launch = parse(&["-i", "contacts", "--install=sales, stock", "-u", "base"])?;
    assert_eq!(launch.install, vec!["contacts", "sales", "stock"]);
    assert_eq!(launch.update, DataUpdate::Only(vec!["base".to_string()]));
    assert!(parse(&["-i"]).is_err(), "a flag without a value");
    Ok(())
}
