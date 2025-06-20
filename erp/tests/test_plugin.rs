extern crate test_plugin;
extern crate test_utilities;

use erp::app::Application;
use erp::plugin::errors::PluginAlreadyRegisteredError;
use std::error::Error;
use test_plugin::TestPlugin;
use test_plugin::TestPlugin2;
use test_plugin::TestPlugin3;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[test]
fn test_load_same_plugin_twice() -> Result<()> {
    let mut app = Application::new_test();

    app.register_plugin(Box::new(TestPlugin {}))
        .expect("Plugin should load");

    let fail = app.plugin_manager.register_plugin(Box::new(TestPlugin {}));
    assert!(fail.is_err());
    let err = fail.unwrap_err();
    assert!(err.is::<PluginAlreadyRegisteredError>());

    Ok(())
}

#[test]
fn test_load_plugin_init_models() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestPlugin {}))
        .expect("Plugin should load");

    // Load plugin should create the model (and so, it should not fail)
    app.load_plugin("test_plugin")?;
    app.model_manager.get_model("sale_order_test");

    // Load the plugin again shouldn't generate any errors
    app.load_plugin("test_plugin")?;

    Ok(())
}

#[test]
fn test_load_plugin_depends() -> Result<()> {
    let mut app = Application::new_test();

    app.register_plugin(Box::new(TestPlugin {}))
        .expect("Plugin should load");
    app.register_plugin(Box::new(TestPlugin2 {}))
        .expect("Plugin should load");
    app.register_plugin(Box::new(TestPlugin3 {}))
        .expect("Plugin should load");

    // Load plugin "test_plugin3" should register the 2 other plugins
    app.load_plugin("test_plugin3")?;

    assert!(app.plugin_manager.is_installed("test_plugin"));
    assert!(app.plugin_manager.is_installed("test_plugin2"));
    assert!(app.plugin_manager.is_installed("test_plugin3"));
    Ok(())
}

#[test]
#[should_panic]
fn test_load_plugin_with_depend_not_register_should_fail() {
    let mut app = Application::new_test();

    app.register_plugin(Box::new(TestPlugin {}))
        .expect("Plugin should load");
    app.register_plugin(Box::new(TestPlugin3 {}))
        .expect("Plugin should load");

    app.load_plugin("test_plugin3")
        .expect("Plugin should not be loaded");
}
