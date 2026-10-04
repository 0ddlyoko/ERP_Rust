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

/// The names of every tag, as the cache `test.tag_names` keeps them.
fn tag_names(env: &mut erp::environment::Environment) -> Result<Vec<String>> {
    let value = env.cached("test.tag_names", "all", |env| {
        let ids = env.search_ids("tag", &erp::search::SearchType::Nothing)?;
        let mut names: Vec<String> = env
            .read("tag", &erp_types::field::MultipleIds::from(ids), &["name"])?
            .iter()
            .filter_map(|row| row.get_option::<&String>("name").cloned())
            .collect();
        names.sort();
        Ok(names)
    })?;
    Ok((*value).clone())
}

/// What a transaction changed and then rolled back never reaches the cache: other requests keep
/// reading what is committed, while the transaction itself saw its own change.
#[test]
fn test_a_rolled_back_change_never_reaches_the_cache() -> Result<()> {
    let mut app = new_app()?;
    app.model_manager
        .shared_caches
        .register("test.tag_names", &["tag"]);
    let mut env = app.new_env_as_option(None)?;
    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp><tag id="kept_tag"><name>kept</name></tag></erp>"#,
    )?;
    let id = data::resolve(&mut env, "seed_plugin.kept_tag")?.unwrap();
    env.close()?;

    let mut env = app.new_env_as_option(None)?;
    let committed = tag_names(&mut env)?;
    env.close()?;
    assert!(committed.contains(&"kept".to_string()), "{committed:?}");

    let mut env = app.new_env_as_option(None)?;
    let mut values = MapOfFields::default();
    values.insert("name", "never committed");
    env.write("tag", &SingleId::from(id), values)?;
    let seen = tag_names(&mut env)?;
    assert!(
        seen.contains(&"never committed".to_string()),
        "its own change: {seen:?}"
    );
    drop(env);

    let mut env = app.new_env_as_option(None)?;
    assert_eq!(
        tag_names(&mut env)?,
        committed,
        "the rollback left the cache as committed"
    );
    Ok(())
}

/// A bounded cache lets go of the value used least recently, not of the oldest one.
#[test]
fn test_a_bounded_cache_keeps_what_is_used() -> Result<()> {
    static BOUNDED_BUILDS: AtomicUsize = AtomicUsize::new(0);
    let mut app = new_app()?;
    app.model_manager
        .shared_caches
        .register_bounded("test.bounded", &["tag"], 2);
    let read = |key: &str| -> Result<usize> {
        let mut env = app.new_env_as_option(None)?;
        let value = env.cached("test.bounded", key, |_| {
            Ok(BOUNDED_BUILDS.fetch_add(1, Ordering::SeqCst))
        })?;
        env.close()?;
        Ok(*value)
    };
    let a = read("a")?;
    let b = read("b")?;
    assert_eq!(read("a")?, a, "used again, so the most recent");
    read("c")?;
    assert_eq!(read("a")?, a, "kept");
    assert_ne!(read("b")?, b, "let go to make room for c, and built again");
    Ok(())
}
