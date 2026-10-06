//! Parameters: a value per key, which plugins read.

use base::BasePlugin;
use base::models::Parameter;
use erp::Result;
use erp::app::Application;
use erp::types::field::MultipleIds;
use serde_json::json;

/// A value kept under a key is read back, and changed in place; a second parameter for the same
/// key is refused.
#[test]
fn test_a_key_holds_one_value() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.load_plugin("base")?;
    let mut env = app.new_env()?;

    assert_eq!(
        Parameter::<MultipleIds>::value_of(&mut env, "colour")?,
        None
    );
    Parameter::<MultipleIds>::keep(&mut env, "colour", "blue")?;
    Parameter::<MultipleIds>::keep(&mut env, "colour", "green")?;
    assert_eq!(
        Parameter::<MultipleIds>::value_of(&mut env, "colour")?.as_deref(),
        Some("green")
    );

    let error = env
        .sudo()
        .call_rpc(
            "parameter",
            "create",
            &json!({ "values": { "key": "colour", "value": "red" } }),
        )
        .expect_err("refused")
        .to_string();
    assert!(error.contains("\"colour\" is already kept"), "{error}");
    Ok(())
}
