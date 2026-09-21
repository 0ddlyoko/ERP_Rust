//! Searching on a relation, and reading one back.
//!
//! A one2many lives in the children's foreign key and a many2many in a table of pairs, so neither
//! can be read off the row it belongs to. Both used to compare against a column that was not
//! there, and a reasonable-looking domain quietly matched nothing.

use erp::app::Application;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use test_utilities::models::{Invoice, SaleOrder, SaleOrderLine, Tag};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.post_register();
    app
}

fn lines_of(env: &mut erp::environment::Environment, order: u32) -> Result<Vec<u32>> {
    let rows = env.read("sale_order", &SingleId::from(order), &["lines"])?;
    Ok(rows[0]
        .get_option::<&Vec<u32>>("lines")
        .cloned()
        .unwrap_or_default())
}

/// A line created after the one2many was read shows up in it.
#[test]
fn test_o2m_sees_a_later_creation() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let order: MultipleIds =
        env.create_records("sale_order", vec![MapOfFields::new(HashMap::new())])?;
    let order = order.get_ids_ref()[0];

    assert!(lines_of(&mut env, order)?.is_empty(), "rien au départ");

    let mut map = MapOfFields::new(HashMap::new());
    map.insert("order", order);
    let line: MultipleIds = env.create_records("sale_order_line", vec![map])?;
    let line = line.get_ids_ref()[0];

    assert_eq!(lines_of(&mut env, order)?, vec![line]);
    Ok(())
}

#[test]
fn test_o2m_sees_a_later_deletion() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let order: MultipleIds =
        env.create_records("sale_order", vec![MapOfFields::new(HashMap::new())])?;
    let order = order.get_ids_ref()[0];
    let mut map = MapOfFields::new(HashMap::new());
    map.insert("order", order);
    let line: MultipleIds = env.create_records("sale_order_line", vec![map])?;
    let line = line.get_ids_ref()[0];

    assert_eq!(lines_of(&mut env, order)?, vec![line]);
    env.delete("sale_order_line", &SingleId::from(line))?;
    assert!(lines_of(&mut env, order)?.is_empty());
    Ok(())
}

#[test]
fn test_o2m_across_environments() -> Result<()> {
    let app = new_app();
    let order = {
        let mut env = app.new_env()?;
        let order: MultipleIds =
            env.create_records("sale_order", vec![MapOfFields::new(HashMap::new())])?;
        let order = order.get_ids_ref()[0];
        let _ = lines_of(&mut env, order)?;
        env.close()?;
        order
    };
    {
        let mut env = app.new_env()?;
        let mut map = MapOfFields::new(HashMap::new());
        map.insert("order", order);
        let _: MultipleIds = env.create_records("sale_order_line", vec![map])?;
        env.close()?;
    }
    let mut env = app.new_env()?;
    assert_eq!(lines_of(&mut env, order)?.len(), 1);
    Ok(())
}

fn tags_of(env: &mut erp::environment::Environment, invoice: u32) -> Result<Vec<u32>> {
    let rows = env.read("invoice", &SingleId::from(invoice), &["tags"])?;
    Ok(rows[0]
        .get_option::<&Vec<u32>>("tags")
        .cloned()
        .unwrap_or_default())
}
fn invoices_of(env: &mut erp::environment::Environment, tag: u32) -> Result<Vec<u32>> {
    let rows = env.read("tag", &SingleId::from(tag), &["invoices"])?;
    Ok(rows[0]
        .get_option::<&Vec<u32>>("invoices")
        .cloned()
        .unwrap_or_default())
}
fn mk(env: &mut erp::environment::Environment, model: &str) -> Result<u32> {
    let ids: MultipleIds = env.create_records(model, vec![MapOfFields::new(HashMap::new())])?;
    Ok(ids.get_ids_ref()[0])
}

#[test]
fn test_o2m_reparenting() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let (a, b) = (mk(&mut env, "sale_order")?, mk(&mut env, "sale_order")?);
    let mut map = MapOfFields::new(HashMap::new());
    map.insert("order", a);
    let line: MultipleIds = env.create_records("sale_order_line", vec![map])?;
    let line = line.get_ids_ref()[0];

    assert_eq!(lines_of(&mut env, a)?, vec![line]);
    assert!(lines_of(&mut env, b)?.is_empty());

    let mut map = MapOfFields::new(HashMap::new());
    map.insert("order", b);
    env.write("sale_order_line", &SingleId::from(line), map)?;

    assert!(lines_of(&mut env, a)?.is_empty());
    assert_eq!(lines_of(&mut env, b)?, vec![line]);
    Ok(())
}

#[test]
fn test_o2m_after_detaching() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let a = mk(&mut env, "sale_order")?;
    let mut map = MapOfFields::new(HashMap::new());
    map.insert("order", a);
    let line: MultipleIds = env.create_records("sale_order_line", vec![map])?;
    let line = line.get_ids_ref()[0];
    assert_eq!(lines_of(&mut env, a)?, vec![line]);

    let mut map = MapOfFields::new(HashMap::new());
    map.insert_none("order");
    env.write("sale_order_line", &SingleId::from(line), map)?;
    assert!(lines_of(&mut env, a)?.is_empty());
    Ok(())
}

#[test]
fn test_m2m_mirror_already_cached() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let (invoice, tag) = (mk(&mut env, "invoice")?, mk(&mut env, "tag")?);

    assert!(
        invoices_of(&mut env, tag)?.is_empty(),
        "miroir chargé en cache"
    );

    let mut map = MapOfFields::new(HashMap::new());
    map.insert("tags", vec![tag]);
    env.write("invoice", &SingleId::from(invoice), map)?;

    assert_eq!(invoices_of(&mut env, tag)?, vec![invoice]);
    Ok(())
}

#[test]
fn test_writing_the_o2m_updates_the_m2o() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let a = mk(&mut env, "sale_order")?;
    let line = mk(&mut env, "sale_order_line")?;

    let rows = env.read("sale_order_line", &SingleId::from(line), &["order"])?;
    assert!(
        rows[0].get_option::<&u32>("order").is_none(),
        "détachée au départ"
    );

    let mut map = MapOfFields::new(HashMap::new());
    map.insert("lines", vec![line]);
    env.write("sale_order", &SingleId::from(a), map)?;

    let rows = env.read("sale_order_line", &SingleId::from(line), &["order"])?;
    assert_eq!(rows[0].get_option::<&u32>("order"), Some(&a));
    Ok(())
}

#[test]
fn test_o2m_with_only_one_side_cached() -> Result<()> {
    let app = new_app();
    let (a, b, line) = {
        let mut env = app.new_env()?;
        let (a, b) = (mk(&mut env, "sale_order")?, mk(&mut env, "sale_order")?);
        let mut map = MapOfFields::new(HashMap::new());
        map.insert("order", a);
        let line: MultipleIds = env.create_records("sale_order_line", vec![map])?;
        env.close()?;
        (a, b, line.get_ids_ref()[0])
    };

    let mut env = app.new_env()?;
    // Seul `a` est chargé ; `b` ne l'est pas.
    assert_eq!(lines_of(&mut env, a)?, vec![line]);

    let mut map = MapOfFields::new(HashMap::new());
    map.insert("order", b);
    env.write("sale_order_line", &SingleId::from(line), map)?;

    assert!(lines_of(&mut env, a)?.is_empty());
    assert_eq!(lines_of(&mut env, b)?, vec![line]);
    Ok(())
}

#[test]
fn test_m2m_written_from_the_mirror() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let (invoice, tag) = (mk(&mut env, "invoice")?, mk(&mut env, "tag")?);

    assert!(
        tags_of(&mut env, invoice)?.is_empty(),
        "côté facture chargé"
    );

    let mut map = MapOfFields::new(HashMap::new());
    map.insert("invoices", vec![invoice]);
    env.write("tag", &SingleId::from(tag), map)?;

    assert_eq!(tags_of(&mut env, invoice)?, vec![tag]);
    Ok(())
}

#[test]
fn test_o2m_after_a_flush() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let a = mk(&mut env, "sale_order")?;
    assert!(lines_of(&mut env, a)?.is_empty());

    let mut map = MapOfFields::new(HashMap::new());
    map.insert("order", a);
    let line: MultipleIds = env.create_records("sale_order_line", vec![map])?;
    env.save_all_to_db()?;

    assert_eq!(lines_of(&mut env, a)?, vec![line.get_ids_ref()[0]]);
    Ok(())
}

#[test]
fn test_writing_the_o2m_reaches_the_database() -> Result<()> {
    let app = new_app();
    let (a, line) = {
        let mut env = app.new_env()?;
        let a = mk(&mut env, "sale_order")?;
        let line = mk(&mut env, "sale_order_line")?;
        let mut map = MapOfFields::new(HashMap::new());
        map.insert("lines", vec![line]);
        env.write("sale_order", &SingleId::from(a), map)?;
        env.close()?;
        (a, line)
    };

    let mut env = app.new_env()?;
    let rows = env.read("sale_order_line", &SingleId::from(line), &["order"])?;
    assert_eq!(rows[0].get_option::<&u32>("order"), Some(&a));
    assert_eq!(lines_of(&mut env, a)?, vec![line]);
    Ok(())
}

#[test]
fn test_targeted_flush_of_an_o2m() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let a = mk(&mut env, "sale_order")?;
    let line = mk(&mut env, "sale_order_line")?;
    let mut map = MapOfFields::new(HashMap::new());
    map.insert("lines", vec![line]);
    env.write("sale_order", &SingleId::from(a), map)?;

    // Flush ciblé : seul "lines" est demandé.
    env.save_fields_to_db("sale_order", &["lines"])?;

    // Ce que la base en dit, sans passer par le cache.
    let found = env.search_ids(
        "sale_order_line",
        &erp_search_code_gen::make_domain!([("order", "=", a)]),
    )?;
    assert_eq!(found, vec![line]);
    Ok(())
}

#[test]
fn test_searching_on_an_unflushed_o2m() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let a = mk(&mut env, "sale_order")?;
    let line = mk(&mut env, "sale_order_line")?;

    let mut map = MapOfFields::new(HashMap::new());
    map.insert("lines", vec![line]);
    env.write("sale_order", &SingleId::from(a), map)?;

    let found = env.search_ids(
        "sale_order",
        &erp_search_code_gen::make_domain!([("lines", "in", vec![line])]),
    )?;
    assert_eq!(found, vec![a]);
    Ok(())
}

#[test]
fn test_searching_on_an_o2m_written_through_the_m2o() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let a = mk(&mut env, "sale_order")?;
    let line = mk(&mut env, "sale_order_line")?;

    let mut map = MapOfFields::new(HashMap::new());
    map.insert("order", a);
    env.write("sale_order_line", &SingleId::from(line), map)?;

    let found = env.search_ids(
        "sale_order",
        &erp_search_code_gen::make_domain!([("lines", "in", vec![line])]),
    )?;
    assert_eq!(found, vec![a]);
    Ok(())
}

#[test]
fn test_searching_on_a_committed_o2m() -> Result<()> {
    let app = new_app();
    let (a, line) = {
        let mut env = app.new_env()?;
        let a = mk(&mut env, "sale_order")?;
        let line = mk(&mut env, "sale_order_line")?;
        let mut map = MapOfFields::new(HashMap::new());
        map.insert("order", a);
        env.write("sale_order_line", &SingleId::from(line), map)?;
        env.close()?;
        (a, line)
    };

    let mut env = app.new_env()?;
    let found = env.search_ids(
        "sale_order",
        &erp_search_code_gen::make_domain!([("lines", "in", vec![line])]),
    )?;
    assert_eq!(found, vec![a]);
    Ok(())
}

#[test]
fn test_searching_through_an_o2m_path() -> Result<()> {
    let app = new_app();
    let (a, _line) = {
        let mut env = app.new_env()?;
        let a = mk(&mut env, "sale_order")?;
        let mut map = MapOfFields::new(HashMap::new());
        map.insert("order", a);
        map.insert("price", 77);
        let line: MultipleIds = env.create_records("sale_order_line", vec![map])?;
        env.close()?;
        (a, line.get_ids_ref()[0])
    };

    let mut env = app.new_env()?;
    let found = env.search_ids(
        "sale_order",
        &erp_search_code_gen::make_domain!([("lines.price", "=", 77)]),
    )?;
    assert_eq!(found, vec![a]);
    Ok(())
}

#[test]
fn test_searching_on_a_m2m_field_directly() -> Result<()> {
    let app = new_app();
    let (invoice, tag) = {
        let mut env = app.new_env()?;
        let (invoice, tag) = (mk(&mut env, "invoice")?, mk(&mut env, "tag")?);
        let mut map = MapOfFields::new(HashMap::new());
        map.insert("tags", vec![tag]);
        env.write("invoice", &SingleId::from(invoice), map)?;
        env.close()?;
        (invoice, tag)
    };

    let mut env = app.new_env()?;
    let found = env.search_ids(
        "invoice",
        &erp_search_code_gen::make_domain!([("tags", "in", vec![tag])]),
    )?;
    assert_eq!(found, vec![invoice]);
    Ok(())
}

#[test]
fn test_searching_on_a_m2o_field_directly() -> Result<()> {
    let app = new_app();
    let (a, line) = {
        let mut env = app.new_env()?;
        let a = mk(&mut env, "sale_order")?;
        let mut map = MapOfFields::new(HashMap::new());
        map.insert("order", a);
        let line: MultipleIds = env.create_records("sale_order_line", vec![map])?;
        env.close()?;
        (a, line.get_ids_ref()[0])
    };

    let mut env = app.new_env()?;
    let found = env.search_ids(
        "sale_order_line",
        &erp_search_code_gen::make_domain!([("order", "=", a)]),
    )?;
    assert_eq!(found, vec![line]);
    Ok(())
}

/// Negation asks for the records a relation does *not* reach, which a negated subquery alone
/// would miss: a record with no link at all appears in neither side of it.
#[test]
fn test_searching_for_what_a_relation_does_not_reach() -> Result<()> {
    let app = new_app();
    let (_linked, alone, line) = {
        let mut env = app.new_env()?;
        let linked = mk(&mut env, "sale_order")?;
        let alone = mk(&mut env, "sale_order")?;
        let mut map = MapOfFields::new(HashMap::new());
        map.insert("order", linked);
        let line: MultipleIds = env.create_records("sale_order_line", vec![map])?;
        env.close()?;
        (linked, alone, line.get_ids_ref()[0])
    };

    let mut env = app.new_env()?;
    let mut found = env.search_ids(
        "sale_order",
        &erp_search_code_gen::make_domain!([("lines", "not in", vec![line])]),
    )?;
    found.sort_unstable();
    assert_eq!(
        found,
        vec![alone],
        "the order with no line at all must be among them"
    );
    Ok(())
}

/// Against nothing, the question is whether the relation reaches anything at all.
#[test]
fn test_searching_for_an_empty_relation() -> Result<()> {
    let app = new_app();
    let (linked, alone) = {
        let mut env = app.new_env()?;
        let linked = mk(&mut env, "sale_order")?;
        let alone = mk(&mut env, "sale_order")?;
        let mut map = MapOfFields::new(HashMap::new());
        map.insert("order", linked);
        let _: MultipleIds = env.create_records("sale_order_line", vec![map])?;
        env.close()?;
        (linked, alone)
    };

    let mut env = app.new_env()?;
    assert_eq!(
        env.search_ids(
            "sale_order",
            &erp_search_code_gen::make_domain!([("lines", "=", None::<u32>)])
        )?,
        vec![alone],
        "orders with no line"
    );
    assert_eq!(
        env.search_ids(
            "sale_order",
            &erp_search_code_gen::make_domain!([("lines", "!=", None::<u32>)])
        )?,
        vec![linked],
        "orders with at least one"
    );
    Ok(())
}

/// The same, on a many2many.
#[test]
fn test_searching_an_empty_many2many() -> Result<()> {
    let app = new_app();
    let (tagged, untagged) = {
        let mut env = app.new_env()?;
        let (tagged, untagged) = (mk(&mut env, "invoice")?, mk(&mut env, "invoice")?);
        let tag = mk(&mut env, "tag")?;
        let mut map = MapOfFields::new(HashMap::new());
        map.insert("tags", vec![tag]);
        env.write("invoice", &SingleId::from(tagged), map)?;
        env.close()?;
        (tagged, untagged)
    };

    let mut env = app.new_env()?;
    assert_eq!(
        env.search_ids(
            "invoice",
            &erp_search_code_gen::make_domain!([("tags", "=", None::<u32>)])
        )?,
        vec![untagged]
    );
    assert_eq!(
        env.search_ids(
            "invoice",
            &erp_search_code_gen::make_domain!([("tags", "!=", None::<u32>)])
        )?,
        vec![tagged]
    );
    Ok(())
}

/// An operator that means nothing against a set is refused rather than answered wrongly.
#[test]
fn test_an_operator_that_does_not_apply_to_a_relation() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    mk(&mut env, "sale_order")?;

    let refused = env.search_ids(
        "sale_order",
        &erp_search_code_gen::make_domain!([("lines", ">", 3)]),
    );
    assert!(
        refused.is_err(),
        "greater-than has no meaning against a set"
    );
    Ok(())
}
