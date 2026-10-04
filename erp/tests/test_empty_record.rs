//! An empty many2one reads as an empty record: still a recordset, whose fields read as their
//! type's default, whose relations are empty in turn, and where writing saves nothing.

use erp::app::Application;
use erp::environment::Environment;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use serde_json::json;
use std::error::Error;
use test_utilities::models::{SaleOrder, SaleOrderLine, SaleOrderState, Tag};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.post_register();
    app
}

fn line_without_order(env: &mut Environment) -> Result<SaleOrderLine<SingleId>> {
    env.create_new_record_from_map(MapOfFields::default())
}

/// The many2one of a line without an order gives an empty order, read through to the end.
#[test]
fn test_an_empty_many2one_reads_as_an_empty_record() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let line = line_without_order(&mut env)?;

    let order: SaleOrder<SingleId> = line.get_order(&mut env)?;
    assert!(order.is_empty());
    assert_eq!(order.get_optional_id(), None);
    assert_eq!(order.get_name(&mut env)?, "", "a string reads as empty");
    assert_eq!(*order.get_total_price(&mut env)?, 0, "a number reads as 0");
    assert_eq!(
        *order.get_state(&mut env)?,
        SaleOrderState::Empty,
        "an enum reads as empty"
    );
    let lines: SaleOrderLine<MultipleIds> = order.get_lines(&mut env)?;
    assert!(lines.id.is_empty(), "its relations are empty in turn");
    let tags: Tag<MultipleIds> = order.get_tags(&mut env)?;
    assert!(tags.get_name(&mut env)?.is_empty());
    assert_eq!(order.id.into_iter().count(), 0, "nothing to loop over");
    Ok(())
}

/// Writing to an empty record saves nothing, and is no mistake.
#[test]
fn test_writing_to_an_empty_record_saves_nothing() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let line = line_without_order(&mut env)?;
    let order: SaleOrder<SingleId> = line.get_order(&mut env)?;

    order.set_name("nobody's".to_string(), &mut env)?;
    let mut values = MapOfFields::default();
    values.insert("name", "nobody's either");
    env.write("sale_order", &SingleId::empty(), values)?;
    env.call_rpc(
        "sale_order",
        "write",
        &json!({"ids": [0], "values": {"name": "still nobody's"}}),
    )?;
    assert_eq!(
        env.count("sale_order", &erp_search::SearchType::Nothing)?,
        0
    );
    Ok(())
}

/// A reference to id 0 points nowhere: saved as empty, without an error.
#[test]
fn test_a_reference_to_zero_is_empty() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let mut values = MapOfFields::default();
    values.insert("name", "real");
    let order: SaleOrder<SingleId> = env.create_new_record_from_map(values)?;

    let mut values = MapOfFields::default();
    values.insert("order", 0u32);
    let line: SaleOrderLine<SingleId> = env.create_new_record_from_map(values)?;
    assert!(line.get_order::<SaleOrder<_>>(&mut env)?.is_empty());

    line.set_order(&order, &mut env)?;
    env.call_rpc(
        "sale_order_line",
        "write",
        &json!({"ids": [line.get_id()], "values": {"order": 0}}),
    )?;
    assert!(line.get_order::<SaleOrder<_>>(&mut env)?.is_empty());
    assert!(order.get_lines::<SaleOrderLine<_>>(&mut env)?.id.is_empty());
    Ok(())
}

/// The empty value of an enum is no value: a required field refuses it.
#[test]
fn test_an_empty_enum_is_no_value() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let mut values = MapOfFields::default();
    values.insert("name", "real");
    let order: SaleOrder<SingleId> = env.create_new_record_from_map(values)?;
    assert_eq!(*order.get_state(&mut env)?, SaleOrderState::Draft);

    let refused = order.set_state(SaleOrderState::Empty, &mut env);
    assert!(
        refused
            .expect_err("refused")
            .to_string()
            .contains("\"state\" of model \"sale_order\" is required")
    );
    Ok(())
}
