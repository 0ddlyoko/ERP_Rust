//! Values kept across requests, forgotten when the records they were built from change.

use base::BasePlugin;
use erp::app::Application;
use erp::data;
use erp_types::field::SingleId;
use erp_types::model::MapOfFields;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use test_utilities::{SeedPlugin, TestLibPlugin};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

static BUILDS: AtomicUsize = AtomicUsize::new(0);

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(SeedPlugin {}))?;
    app.load_plugin("seed_plugin")?;
    app.model_manager
        .shared_caches
        .register("test.tags", &["tag"]);
    Ok(app)
}

fn read_cached(app: &Application) -> Result<usize> {
    let mut env = app.new_env_as_option(None)?;
    let value = env.cached("test.tags", "all", |_| {
        Ok(BUILDS.fetch_add(1, Ordering::SeqCst))
    })?;
    env.close()?;
    Ok(*value)
}

#[test]
fn test_a_value_is_kept_until_its_records_change() -> Result<()> {
    let app = new_app()?;
    let first = read_cached(&app)?;
    assert_eq!(read_cached(&app)?, first, "kept");

    let mut env = app.new_env_as_option(None)?;
    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp><tag id="new_tag"><name>new</name></tag></erp>"#,
    )?;
    let id = data::resolve(&mut env, "seed_plugin.new_tag")?.unwrap();
    let mut values = MapOfFields::default();
    values.insert("name", "renamed");
    env.write("tag", &SingleId::from(id), values)?;
    env.close()?;

    assert_ne!(read_cached(&app)?, first, "built again");
    Ok(())
}

#[test]
fn test_an_undeclared_cache_is_an_error() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    assert!(env.cached("nobody.declared", "x", |_| Ok(0)).is_err());
    Ok(())
}
