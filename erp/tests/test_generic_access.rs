use erp::app::Application;
use erp_search_code_gen::make_domain;
use erp_types::field::{Decimal, IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use test_utilities::models::{Invoice, SaleOrder, SaleOrderLine};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.post_register();
    app
}

/// Create and read back knowing nothing but the model name — the shape an API handler needs.
#[test]
fn test_create_and_read_by_model_name() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "INV-001");
    map.insert("amount_untaxed", Decimal::from_str("42.50")?);
    let ids = env.create_records("invoice", vec![map])?;
    assert_eq!(ids.get_ids_ref().len(), 1);

    let rows = env.read("invoice", &ids, &["name", "amount_untaxed"])?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<&String>("name"), &"INV-001".to_string());
    assert_eq!(
        rows[0].get::<&Decimal>("amount_untaxed"),
        &Decimal::from_str("42.50")?
    );
    Ok(())
}

/// `read` always carries the id back, whether or not it was asked for.
#[test]
fn test_read_includes_the_id() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let ids = env.create_records("invoice", vec![MapOfFields::new(HashMap::new())])?;
    let rows = env.read("invoice", &ids, &["name"])?;
    assert_eq!(
        rows[0].get::<&u32>("id"),
        ids.get_ids_ref().first().unwrap()
    );
    Ok(())
}

/// A NULL reads back as `None` instead of raising, which is what separates the generic reader
/// from `dyn Model::get`.
#[test]
fn test_null_field_reads_as_none() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert_none("signed_on");
    let ids = env.create_records("invoice", vec![map])?;

    let rows = env.read("invoice", &ids, &["signed_on"])?;
    assert!(
        rows[0]
            .get_option::<&erp_types::field::NaiveDate>("signed_on")
            .is_none(),
        "a NULL must read back as None"
    );
    Ok(())
}

/// Pins down a wart worth knowing about: declaring a field `Option<T>` does **not** make it
/// nullable. Every field carries a default, because the default doubles as the type tag
/// (`erp_internal_types/src/field.rs:8`), so an unset `Option<NaiveDate>` stores 1970-01-01.
/// `Option<T>` currently only changes the generated accessor's signature.
#[test]
fn test_unset_optional_field_still_gets_a_default() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let ids = env.create_records("invoice", vec![MapOfFields::new(HashMap::new())])?;
    let rows = env.read("invoice", &ids, &["signed_on"])?;
    assert!(
        rows[0]
            .get_option::<&erp_types::field::NaiveDate>("signed_on")
            .is_some(),
        "today an unset optional field is filled with its type default, not NULL"
    );
    Ok(())
}

/// Reading several records returns one map per id, in order.
#[test]
fn test_read_many_records_in_order() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let maps: Vec<MapOfFields> = ["A", "B", "C"]
        .iter()
        .map(|name| {
            let mut map = MapOfFields::new(HashMap::new());
            map.insert("name", *name);
            map
        })
        .collect();
    let ids = env.create_records("invoice", maps)?;
    assert_eq!(ids.get_ids_ref().len(), 3);

    let rows = env.read("invoice", &ids, &["name"])?;
    let names: Vec<&String> = rows.iter().map(|row| row.get::<&String>("name")).collect();
    assert_eq!(
        names,
        vec![&"A".to_string(), &"B".to_string(), &"C".to_string()]
    );
    Ok(())
}

/// Searching by model name, without a compile-time type.
#[test]
fn test_search_ids_by_model_name() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "findme");
    env.create_records("invoice", vec![map])?;

    let ids = env.search_ids("invoice", &make_domain!([("name", "=", "findme")]))?;
    assert_eq!(ids.len(), 1);

    let none = env.search_ids("invoice", &make_domain!([("name", "=", "absent")]))?;
    assert!(none.is_empty());
    Ok(())
}

/// A model name that never existed is reported, not fatal.
#[test]
fn test_unknown_model_name_is_an_error() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let err = env
        .search_ids("not_a_model", &make_domain!([("name", "=", "x")]))
        .unwrap_err();
    assert!(
        err.to_string().contains("not_a_model"),
        "the error must name the offending model, got: {err}"
    );
    Ok(())
}

/// Same for a field name that does not exist on the model.
#[test]
fn test_unknown_field_name_is_an_error() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let ids = env.create_records("invoice", vec![MapOfFields::new(HashMap::new())])?;
    let err = env.read("invoice", &ids, &["not_a_field"]).unwrap_err();
    assert!(
        err.to_string().contains("not_a_field"),
        "the error must name the offending field, got: {err}"
    );
    Ok(())
}

/// Generic access still drives the compute engine.
#[test]
fn test_read_triggers_computed_fields() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("price", 6);
    map.insert("amount", 7);
    let ids = env.create_records("sale_order_line", vec![map])?;

    let rows = env.read("sale_order_line", &ids, &["total_price"])?;
    assert_eq!(rows[0].get::<&i32>("total_price"), &42);
    Ok(())
}

/// The typed API and the by-name API address the same records.
#[test]
fn test_typed_and_generic_agree() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "shared");
    env.create_records("invoice", vec![map])?;

    let typed: Invoice<MultipleIds> = env.search(&make_domain!([("name", "=", "shared")]))?;
    let generic = env.search_ids("invoice", &make_domain!([("name", "=", "shared")]))?;
    assert_eq!(typed.id.get_ids_ref(), &generic);

    let single: Invoice<SingleId> = env.get_record(generic[0].into());
    let rows = env.read("invoice", &single.id, &["name"])?;
    assert_eq!(rows[0].get::<&String>("name"), &"shared".to_string());
    Ok(())
}
