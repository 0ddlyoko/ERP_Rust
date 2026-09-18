use base::BasePlugin;
use erp::app::Application;
use erp::data;
use erp_search_code_gen::make_domain;
use erp_types::field::{Decimal, IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use test_utilities::models::{SaleOrder, SaleOrderLine};
use test_utilities::{SeedPlugin, TestLibPlugin};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// `model_data` lives in `base`, so both plugins are needed.
fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(SeedPlugin {}))?;
    app.load_plugin("seed_plugin")?;
    Ok(app)
}

/// Records declared in a data file exist once the plugin is loaded.
#[test]
fn test_records_are_created_from_the_file() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    let orders: SaleOrder<MultipleIds> =
        env.search(&make_domain!([("name", "=", "Seeded order")]))?;
    assert_eq!(orders.id.get_ids_ref().len(), 1);
    Ok(())
}

/// A field is read according to its declared type, not as text.
#[test]
fn test_values_are_parsed_by_kind() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    let invoices: SaleOrder<MultipleIds> =
        env.search(&make_domain!([("name", "=", "Seeded order")]))?;
    assert!(!invoices.id.is_empty());

    let ids = env.search_ids("invoice", &make_domain!([("name", "=", "Untouchable")]))?;
    let rows = env.read(
        "invoice",
        &MultipleIds::from(ids),
        &["amount_untaxed", "due_date"],
    )?;
    assert_eq!(
        rows[0].get::<&Decimal>("amount_untaxed"),
        &Decimal::from_str("1234.56")?,
        "a decimal must be parsed as a decimal, exactly"
    );
    assert!(
        rows[0]
            .get_option::<&erp_types::field::NaiveDate>("due_date")
            .is_some()
    );
    Ok(())
}

/// `ref=` resolves an external identifier to the record it designates.
#[test]
fn test_references_are_resolved() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    let order_id =
        data::resolve(&mut env, "seed_plugin.main_order")?.expect("the order must be registered");
    let lines: SaleOrderLine<MultipleIds> = env.search(&make_domain!([("price", "=", 7)]))?;
    assert_eq!(lines.id.get_ids_ref().len(), 1);

    let rows = env.read("sale_order_line", &lines.id, &["order"])?;
    assert_eq!(
        rows[0].get::<&u32>("order"),
        &order_id,
        "the line must point at the order the reference named"
    );
    Ok(())
}

/// The registry maps the external identifier to a technical id.
#[test]
fn test_external_ids_are_registered() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    assert!(data::resolve(&mut env, "seed_plugin.main_order")?.is_some());
    assert!(data::resolve(&mut env, "seed_plugin.first_line")?.is_some());
    assert!(
        data::resolve(&mut env, "seed_plugin.never_declared")?.is_none(),
        "an unknown identifier resolves to nothing rather than failing"
    );
    Ok(())
}

/// Loading the same file twice must not duplicate anything, and must not move the ids.
#[test]
fn test_loading_twice_is_idempotent() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    let before = data::resolve(&mut env, "seed_plugin.main_order")?.unwrap();
    assert_eq!(
        env.count("sale_order", &make_domain!([("name", "=", "Seeded order")]))?,
        1
    );

    data::load(
        &mut env,
        "seed_plugin",
        include_str!("../test_utilities/data/orders.xml"),
    )?;

    assert_eq!(
        env.count("sale_order", &make_domain!([("name", "=", "Seeded order")]))?,
        1,
        "a second load must not create a second record"
    );
    assert_eq!(
        data::resolve(&mut env, "seed_plugin.main_order")?,
        Some(before),
        "the technical id must not move"
    );
    Ok(())
}

/// A second load updates the record, unless it is protected.
#[test]
fn test_reload_updates_unless_protected() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    // Stand in for a user edit.
    let order_id = data::resolve(&mut env, "seed_plugin.main_order")?.unwrap();
    let mut edit = MapOfFields::new(HashMap::new());
    edit.insert("name", "Edited by hand");
    env.write("sale_order", &SingleId::from(order_id), edit)?;

    let invoice_id = data::resolve(&mut env, "seed_plugin.reference_invoice")?.unwrap();
    let mut edit = MapOfFields::new(HashMap::new());
    edit.insert("name", "Edited too");
    env.write("invoice", &SingleId::from(invoice_id), edit)?;

    data::load(
        &mut env,
        "seed_plugin",
        include_str!("../test_utilities/data/orders.xml"),
    )?;

    let rows = env.read("sale_order", &SingleId::from(order_id), &["name"])?;
    assert_eq!(
        rows[0].get::<&String>("name"),
        &"Seeded order".to_string(),
        "an unprotected record is brought back in line"
    );

    let rows = env.read("invoice", &SingleId::from(invoice_id), &["name"])?;
    assert_eq!(
        rows[0].get::<&String>("name"),
        &"Edited too".to_string(),
        "noupdate must leave the record to its owner"
    );
    Ok(())
}

/// A reference to something that was never declared is reported, and names it.
#[test]
fn test_unknown_reference_is_reported() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    let err = data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="orphan" model="sale_order_line">
                <field name="order" ref="nowhere.at_all"/>
            </record>
        </erp>"#,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("nowhere.at_all"),
        "the error must name the missing reference, got: {err}"
    );
    Ok(())
}

/// Malformed XML is reported rather than silently skipped.
#[test]
fn test_malformed_xml_is_reported() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    let err = data::load(&mut env, "seed_plugin", "<erp><record>").unwrap_err();
    assert!(err.to_string().contains("seed_plugin"), "got: {err}");
    Ok(())
}

/// Computed fields still run on records that came from a file.
#[test]
fn test_computed_fields_run_on_loaded_records() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    let order_id = data::resolve(&mut env, "seed_plugin.main_order")?.unwrap();
    let order: SaleOrder<SingleId> = env.get_record(order_id.into());
    assert_eq!(
        *order.get_total_price(&mut env)?,
        42,
        "7 * 6 from the seeded line"
    );
    Ok(())
}

/// The invoice model is untouched by the sale order file beyond what it declares.
#[test]
fn test_only_declared_records_are_created() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    assert_eq!(env.count("invoice", &make_domain!([]))?, 1);
    assert_eq!(env.count("sale_order", &make_domain!([]))?, 1);
    assert_eq!(env.count("sale_order_line", &make_domain!([]))?, 1);
    Ok(())
}
