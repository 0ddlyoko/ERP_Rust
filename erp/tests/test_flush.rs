use erp::app::Application;
use erp_search_code_gen::make_domain;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use test_utilities::models::{SaleOrder, SaleOrderLine};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.post_register();
    app
}

/// `close()` must flush dirty cache entries, not just commit the transaction.
///
/// The write below is deliberately never followed by a search on "amount": `search` flushes the
/// fields of its own domain, which would hide a no-op flush.
#[test]
fn test_close_flushes_dirty_fields_to_db() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("amount", 42);
    let _line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
    env.close()?;

    let mut env = app.new_env()?;
    let line: SaleOrderLine<MultipleIds> = env.search(&make_domain!([("amount", "=", 42)]))?;
    assert!(!line.id.is_empty());
    line.set_amount(69, &mut env)?;
    env.close()?;

    let mut env = app.new_env()?;
    let updated: SaleOrderLine<MultipleIds> = env.search(&make_domain!([("amount", "=", 69)]))?;
    assert!(
        !updated.id.is_empty(),
        "close() must flush dirty fields to the database"
    );
    let stale: SaleOrderLine<MultipleIds> = env.search(&make_domain!([("amount", "=", 42)]))?;
    assert!(
        stale.id.is_empty(),
        "the value replaced before close() must no longer be in the database"
    );
    Ok(())
}

/// Dropping an environment without closing it must still roll everything back.
#[test]
fn test_drop_without_close_discards_dirty_fields() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("amount", 42);
    let _line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
    env.close()?;

    let mut env = app.new_env()?;
    let line: SaleOrderLine<MultipleIds> = env.search(&make_domain!([("amount", "=", 42)]))?;
    line.set_amount(69, &mut env)?;
    drop(env);

    let mut env = app.new_env()?;
    let stale: SaleOrderLine<MultipleIds> = env.search(&make_domain!([("amount", "=", 42)]))?;
    assert!(!stale.id.is_empty(), "a dropped environment must roll back");
    Ok(())
}

/// `save_model_to_db` drives `call_computed_method_on_all_fields`, whose loop used to break while
/// work remained and spin when none did.
#[test]
fn test_save_model_to_db_computes_every_pending_field() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("price", 5);
    map.insert("amount", 7);
    let line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;

    // total_price is computed from price * amount, and is flagged to_recompute by the creation.
    env.save_model_to_db("sale_order_line")?;

    assert_eq!(*line.get_total_price(&mut env)?, 35);
    Ok(())
}

/// A compute that cascades across models must also settle in a single `save_model_to_db`.
#[test]
fn test_save_model_to_db_computes_across_relations() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "order");
    let order: SaleOrder<SingleId> = env.create_new_record_from_map(map)?;

    for (price, amount) in [(2, 3), (4, 5)] {
        let mut map: MapOfFields = MapOfFields::new(HashMap::new());
        map.insert("price", price);
        map.insert("amount", amount);
        map.insert("order", order.get_id());
        let _line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
    }

    env.save_model_to_db("sale_order_line")?;
    env.save_model_to_db("sale_order")?;

    // 2*3 + 4*5
    assert_eq!(*order.get_total_price(&mut env)?, 26);
    Ok(())
}
