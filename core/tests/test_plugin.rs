extern crate test_plugin;
extern crate utilities;
mod models;

use std::error::Error;
use core::plugin::errors::PluginAlreadyRegisteredError;
use core::app::Application;
use test_plugin::TestPlugin;

#[test]
fn test_load_same_plugin_twice() -> Result<(), Box<dyn Error>> {
    let mut app = Application::new(utilities::default_config()?);

    app.register_plugin(Box::new(TestPlugin {})).expect("Plugin should load");

    let fail = app.plugin_manager.register_plugin(Box::new(TestPlugin {}));
    assert!(fail.is_err());
    let err = fail.unwrap_err();
    assert!(err.is::<PluginAlreadyRegisteredError>());

    Ok(())
}

#[test]
fn test_load_plugin_init_models() -> Result<(), Box<dyn Error>> {
    let mut app = Application::new(utilities::default_config()?);
    app.register_plugin(Box::new(TestPlugin {})).expect("Plugin should load");

    let model = app.model_manager.get_model("sale_order_test");
    assert!(model.is_none());

    // Load plugin should create the model
    app.load_plugin("test_plugin")?;
    let model = app.model_manager.get_model("sale_order_test");
    assert!(model.is_some());

    Ok(())
}
