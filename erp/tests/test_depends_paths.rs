//! Dependency paths of more than one segment, across every kind of relation.
use erp::app::Application;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use test_utilities::models::{Invoice, SaleOrder, SaleOrderLine, Tag};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.post_register();
    app
}

fn named(env: &mut erp::environment::Environment, model: &str, name: &str) -> Result<u32> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", name);
    let ids: MultipleIds = env.create_records(model, vec![map])?;
    Ok(*ids.get_ids_ref().first().unwrap())
}

fn line_of(env: &mut erp::environment::Environment, order: u32, price: i32) -> Result<u32> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("price", price);
    map.insert("amount", 1);
    map.insert("order", order);
    let ids: MultipleIds = env.create_records("sale_order_line", vec![map])?;
    Ok(*ids.get_ids_ref().first().unwrap())
}

fn read<'a, T>(
    env: &mut erp::environment::Environment,
    model: &str,
    id: u32,
    field: &str,
) -> Result<T>
where
    T: Clone + 'static,
    for<'b> &'b erp_types::field::FieldType: Into<Option<&'b T>>,
{
    let rows = env.read(model, &SingleId::from(id), &[field])?;
    Ok(rows[0].get::<&T>(field).clone())
}

/// Three segments across a many2one then a one2many.
#[test]
fn test_depends_through_m2o_then_o2m() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let order = named(&mut env, "sale_order", "order")?;
    let first = line_of(&mut env, order, 10)?;
    line_of(&mut env, order, 32)?;

    assert_eq!(
        read::<i32>(&mut env, "sale_order_line", first, "siblings_total")?,
        42,
        "the line must see the prices of its siblings"
    );
    Ok(())
}

/// Changing the far end of that path recomputes the near end.
#[test]
fn test_far_end_of_an_o2m_path_triggers_recompute() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let order = named(&mut env, "sale_order", "order")?;
    let first = line_of(&mut env, order, 10)?;
    let second = line_of(&mut env, order, 32)?;
    assert_eq!(
        read::<i32>(&mut env, "sale_order_line", first, "siblings_total")?,
        42
    );

    let mut change = MapOfFields::new(HashMap::new());
    change.insert("price", 90);
    env.write("sale_order_line", &SingleId::from(second), change)?;

    assert_eq!(
        read::<i32>(&mut env, "sale_order_line", first, "siblings_total")?,
        100,
        "changing a sibling's price must reach the other line"
    );
    Ok(())
}

/// Adding a record at the far end counts too.
#[test]
fn test_adding_at_the_far_end_triggers_recompute() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let order = named(&mut env, "sale_order", "order")?;
    let first = line_of(&mut env, order, 10)?;
    assert_eq!(
        read::<i32>(&mut env, "sale_order_line", first, "siblings_total")?,
        10
    );

    line_of(&mut env, order, 5)?;
    assert_eq!(
        read::<i32>(&mut env, "sale_order_line", first, "siblings_total")?,
        15
    );
    Ok(())
}

/// Three segments across a many2one then a many2many.
#[test]
fn test_depends_through_m2o_then_m2m() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let order = named(&mut env, "sale_order", "order")?;
    let line = line_of(&mut env, order, 10)?;
    let urgent = named(&mut env, "tag", "urgent")?;
    let late = named(&mut env, "tag", "late")?;

    let mut tags = MapOfFields::new(HashMap::new());
    tags.insert("tags", vec![urgent, late]);
    env.write("sale_order", &SingleId::from(order), tags)?;

    assert_eq!(
        read::<String>(&mut env, "sale_order_line", line, "order_tags")?,
        "late,urgent",
        "the line must see the tags of its order"
    );
    Ok(())
}

/// Renaming a tag two hops away reaches the line.
#[test]
fn test_far_end_of_an_m2m_path_triggers_recompute() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let order = named(&mut env, "sale_order", "order")?;
    let line = line_of(&mut env, order, 10)?;
    let urgent = named(&mut env, "tag", "urgent")?;

    let mut tags = MapOfFields::new(HashMap::new());
    tags.insert("tags", vec![urgent]);
    env.write("sale_order", &SingleId::from(order), tags)?;
    assert_eq!(
        read::<String>(&mut env, "sale_order_line", line, "order_tags")?,
        "urgent"
    );

    let mut rename = MapOfFields::new(HashMap::new());
    rename.insert("name", "critical");
    env.write("tag", &SingleId::from(urgent), rename)?;

    assert_eq!(
        read::<String>(&mut env, "sale_order_line", line, "order_tags")?,
        "critical",
        "a rename two relations away must reach the line"
    );
    Ok(())
}

/// Unlinking at the far end reaches it too.
#[test]
fn test_unlinking_two_hops_away_triggers_recompute() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let order = named(&mut env, "sale_order", "order")?;
    let line = line_of(&mut env, order, 10)?;
    let urgent = named(&mut env, "tag", "urgent")?;

    let mut tags = MapOfFields::new(HashMap::new());
    tags.insert("tags", vec![urgent]);
    env.write("sale_order", &SingleId::from(order), tags)?;
    assert_eq!(
        read::<String>(&mut env, "sale_order_line", line, "order_tags")?,
        "urgent"
    );

    let mut cleared = MapOfFields::new(HashMap::new());
    cleared.insert("tags", Vec::<u32>::new());
    env.write("sale_order", &SingleId::from(order), cleared)?;

    assert_eq!(
        read::<String>(&mut env, "sale_order_line", line, "order_tags")?,
        "",
        "removing the link must reach the line"
    );
    Ok(())
}

/// Records on another order are left alone.
#[test]
fn test_recompute_does_not_spill_across_orders() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let first_order = named(&mut env, "sale_order", "first")?;
    let second_order = named(&mut env, "sale_order", "second")?;
    let first_line = line_of(&mut env, first_order, 10)?;
    let second_line = line_of(&mut env, second_order, 7)?;

    assert_eq!(
        read::<i32>(&mut env, "sale_order_line", first_line, "siblings_total")?,
        10
    );
    assert_eq!(
        read::<i32>(&mut env, "sale_order_line", second_line, "siblings_total")?,
        7
    );

    line_of(&mut env, first_order, 90)?;

    assert_eq!(
        read::<i32>(&mut env, "sale_order_line", first_line, "siblings_total")?,
        100
    );
    assert_eq!(
        read::<i32>(&mut env, "sale_order_line", second_line, "siblings_total")?,
        7,
        "the other order must be untouched"
    );
    Ok(())
}
