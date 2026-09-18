use erp::app::Application;
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use test_plugin::TestPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// `sale_order_test` is contributed by two structs, and both declare a compute for `label` and
/// for `replaced`.
fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestPlugin {}))?;
    app.load_plugin("test_plugin")?;
    Ok(app)
}

fn label_of(env: &mut erp::environment::Environment, name: &str, field: &str) -> Result<String> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", name);
    let ids = env.create_records("sale_order_test", vec![map])?;
    let rows = env.read("sale_order_test", &ids, &[field])?;
    Ok(rows[0].get::<&String>(field).clone())
}

/// Both contributors are registered, in order, on the same field.
#[test]
fn test_chain_holds_every_contributor() -> Result<()> {
    let app = new_app()?;
    let model = app.model_manager.get_model("sale_order_test");

    assert_eq!(
        model.compute_chain("label").map(<[_]>::len),
        Some(2),
        "both structs declare a compute for label"
    );
    assert_eq!(
        model.compute_chain("age"),
        None,
        "a field without a compute has no chain"
    );
    assert_eq!(model.compute_chain("not_a_field"), None);
    Ok(())
}

/// The most derived runs first and reaches the base through `super`.
#[test]
fn test_super_reaches_the_overridden_implementation() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    assert_eq!(
        label_of(&mut env, "order", "label")?,
        "base:order+derived",
        "the base must run through super, then the derived appends to its result"
    );
    Ok(())
}

/// Not calling `super` replaces the implementation below.
#[test]
fn test_not_calling_super_replaces() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    assert_eq!(
        label_of(&mut env, "order", "replaced")?,
        "derived",
        "the base implementation must not have run"
    );
    Ok(())
}

/// A chain of one is the common case: `super` must be safe to call and simply do nothing.
#[test]
fn test_super_on_a_single_link_is_a_noop() -> Result<()> {
    let mut app = Application::new_test();
    app.model_manager
        .register_model::<test_utilities::models::SaleOrder<_>>();
    app.model_manager
        .register_model::<test_utilities::models::SaleOrderLine<_>>();
    app.model_manager.post_register();

    let model = app.model_manager.get_model("sale_order_line");
    assert_eq!(model.compute_chain("total_price").map(<[_]>::len), Some(1));

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("price", 3);
    map.insert("amount", 4);
    let ids = env.create_records("sale_order_line", vec![map])?;
    let rows = env.read("sale_order_line", &ids, &["total_price"])?;
    assert_eq!(rows[0].get::<&i32>("total_price"), &12);
    Ok(())
}

/// The chain still recomputes on dependency changes, and stays correct across a commit.
#[test]
fn test_chain_reruns_on_dependency_change() -> Result<()> {
    let app = new_app()?;

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "first");
    let ids = env.create_records("sale_order_test", vec![map])?;
    assert_eq!(
        env.read("sale_order_test", &ids, &["label"])?[0].get::<&String>("label"),
        &"base:first+derived".to_string()
    );
    env.close()?;

    let mut env = app.new_env()?;
    let mut update: MapOfFields = MapOfFields::new(HashMap::new());
    update.insert("name", "second");
    env.write("sale_order_test", &ids, update)?;
    assert_eq!(
        env.read("sale_order_test", &ids, &["label"])?[0].get::<&String>("label"),
        &"base:second+derived".to_string(),
        "changing a dependency must re-run the whole chain"
    );
    Ok(())
}
