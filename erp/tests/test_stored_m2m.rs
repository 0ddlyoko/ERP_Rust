//! A many2many worked out from other fields and kept in its table of pairs: worked out when what
//! it depends on changes, read back without working it out again, and set by hand.

use erp::app::Application;
use erp::environment::Environment;
use erp_search_code_gen::make_domain;
use erp_types::field::{FieldType, IdMode};
use erp_types::model::MapOfFields;
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

fn create(env: &mut Environment, model: &str, values: &[(&str, FieldType)]) -> Result<u32> {
    let mut map = MapOfFields::default();
    for (name, value) in values {
        map.insert_field_type(name, value.clone());
    }
    Ok(env.create_records(model, vec![map])?.get_ids_ref()[0])
}

fn tag(env: &mut Environment, name: &str) -> Result<u32> {
    create(env, "tag", &[("name", FieldType::String(name.to_string()))])
}

fn tags_of(env: &mut Environment, line: u32) -> Result<Vec<u32>> {
    let rows = env.read(
        "sale_order_line",
        &erp_types::field::SingleId::from(line),
        &["tags"],
    )?;
    let mut ids = rows[0]
        .get_option::<&Vec<u32>>("tags")
        .cloned()
        .unwrap_or_default();
    ids.sort_unstable();
    Ok(ids)
}

/// A line takes its order's tags, and follows them as they change.
#[test]
fn test_worked_out_from_what_it_depends_on() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let (urgent, export) = (tag(&mut env, "urgent")?, tag(&mut env, "export")?);
    let order = create(
        &mut env,
        "sale_order",
        &[("tags", FieldType::Refs(vec![urgent]))],
    )?;
    let line = create(
        &mut env,
        "sale_order_line",
        &[("order", FieldType::Ref(order))],
    )?;
    assert_eq!(tags_of(&mut env, line)?, vec![urgent]);

    let mut values = MapOfFields::default();
    values.insert_field_type("tags", FieldType::Refs(vec![urgent, export]));
    env.write(
        "sale_order",
        &erp_types::field::SingleId::from(order),
        values,
    )?;
    assert_eq!(tags_of(&mut env, line)?, vec![urgent, export]);
    Ok(())
}

/// Set by hand — at creation or after — the tags stay until what they depend on changes.
#[test]
fn test_set_by_hand() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let (urgent, export) = (tag(&mut env, "urgent")?, tag(&mut env, "export")?);
    let order = create(
        &mut env,
        "sale_order",
        &[("tags", FieldType::Refs(vec![urgent]))],
    )?;
    let given = create(
        &mut env,
        "sale_order_line",
        &[
            ("order", FieldType::Ref(order)),
            ("tags", FieldType::Refs(vec![export])),
        ],
    )?;
    assert_eq!(tags_of(&mut env, given)?, vec![export], "given at creation");

    let line = create(
        &mut env,
        "sale_order_line",
        &[("order", FieldType::Ref(order))],
    )?;
    let mut values = MapOfFields::default();
    values.insert_field_type("tags", FieldType::Refs(vec![export]));
    env.write(
        "sale_order_line",
        &erp_types::field::SingleId::from(line),
        values,
    )?;
    assert_eq!(tags_of(&mut env, line)?, vec![export], "written by hand");
    Ok(())
}

/// What is kept can be searched, as any many2many.
#[test]
fn test_kept_tags_are_searched() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let urgent = tag(&mut env, "urgent")?;
    let tagged = create(
        &mut env,
        "sale_order",
        &[("tags", FieldType::Refs(vec![urgent]))],
    )?;
    let plain = create(&mut env, "sale_order", &[])?;
    let line = create(
        &mut env,
        "sale_order_line",
        &[("order", FieldType::Ref(tagged))],
    )?;
    create(
        &mut env,
        "sale_order_line",
        &[("order", FieldType::Ref(plain))],
    )?;
    let found = env.search_ids(
        "sale_order_line",
        &make_domain!([("tags.name", "=", "urgent")]),
    )?;
    assert_eq!(found, vec![line]);
    Ok(())
}
