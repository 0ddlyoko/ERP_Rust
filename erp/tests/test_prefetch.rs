//! Reading a field of one record of a recordset loads it for the others.
//!
//! A loop over records reads the same field of each in turn. Loading it for the whole recordset at
//! the first read turns one query per record into one query, which is what a loop needs to stay
//! cheap as the recordset grows.

use erp::app::Application;
use erp::environment::Environment;
use erp_search::SearchType;
use erp_types::field::{FieldType, IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use test_utilities::models::{Invoice, SaleOrder, SaleOrderLine, Tag};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.post_register();
    app
}

/// Tags committed, so that the next environment starts with nothing in its cache.
fn tags(app: &Application, count: usize) -> Result<Vec<u32>> {
    let mut env = app.new_env()?;
    let values = (0..count)
        .map(|i| {
            let mut values = MapOfFields::default();
            values.insert("name", format!("tag {i}"));
            values
        })
        .collect();
    let ids = env.create_records("tag", values)?.ids;
    env.close()?;
    Ok(ids)
}

fn cached(env: &Environment, model_name: &str, field_name: &str, ids: &[u32]) -> usize {
    ids.iter()
        .filter(|id| env.cache.is_field_in_cache(model_name, field_name, **id))
        .count()
}

#[test]
fn test_reading_one_record_of_a_loop_loads_the_others() -> Result<()> {
    let app = new_app();
    let ids = tags(&app, 3)?;

    let mut env = app.new_env()?;
    let found: Tag<MultipleIds> = env.search(&SearchType::Nothing)?;
    let first = found.into_iter().next().expect("three tags");
    first.get_name(&mut env)?;
    assert_eq!(cached(&env, "tag", "name", &ids), 3);
    Ok(())
}

/// Borrowing the recordset rather than consuming it prefetches the same way.
#[test]
fn test_a_borrowed_recordset_prefetches_too() -> Result<()> {
    let app = new_app();
    let ids = tags(&app, 3)?;

    let mut env = app.new_env()?;
    let found: Tag<MultipleIds> = env.search(&SearchType::Nothing)?;
    let mut names = Vec::new();
    for tag in &found {
        names.push(tag.get_name(&mut env)?.clone());
        assert_eq!(
            cached(&env, "tag", "name", &ids),
            3,
            "all loaded at the first read"
        );
    }
    assert_eq!(names, vec!["tag 0", "tag 1", "tag 2"]);
    Ok(())
}

/// A record reached on its own has no recordset, and loads only itself.
#[test]
fn test_a_record_on_its_own_loads_only_itself() -> Result<()> {
    let app = new_app();
    let ids = tags(&app, 3)?;

    let mut env = app.new_env()?;
    let tag: Tag<SingleId> = env.get_record(ids[0].into());
    tag.get_name(&mut env)?;
    assert_eq!(cached(&env, "tag", "name", &ids), 1);
    Ok(())
}

/// A one2many is loaded for every record of the loop at once, like a column.
#[test]
fn test_a_one2many_is_prefetched() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let mut orders = Vec::new();
    for _ in 0..3 {
        let mut values = MapOfFields::default();
        values.insert("name", "Order");
        let order = env
            .create_records("sale_order", vec![values])?
            .get_ids_ref()[0];
        let mut values = MapOfFields::default();
        values.insert("order", FieldType::Ref(order));
        env.create_records("sale_order_line", vec![values])?;
        orders.push(order);
    }
    env.close()?;

    let mut env = app.new_env()?;
    let found: SaleOrder<MultipleIds> = env.search(&SearchType::Nothing)?;
    let first = found.into_iter().next().expect("three orders");
    let lines: SaleOrderLine<MultipleIds> = first.get_lines(&mut env)?;
    assert_eq!(lines.get_ids_ref().len(), 1);
    assert_eq!(cached(&env, "sale_order", "lines", &orders), 3);
    Ok(())
}

/// A value worked out on each read is not worked out for records nobody read.
#[test]
fn test_a_compute_on_read_is_not_prefetched() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let mut values = MapOfFields::default();
    values.insert("name", "Order");
    let order = env
        .create_records("sale_order", vec![values])?
        .get_ids_ref()[0];
    let lines: Vec<u32> = (0..3)
        .map(|_| {
            let mut values = MapOfFields::default();
            values.insert("order", FieldType::Ref(order));
            Ok(env
                .create_records("sale_order_line", vec![values])?
                .get_ids_ref()[0])
        })
        .collect::<Result<_>>()?;
    env.close()?;

    let mut env = app.new_env()?;
    let found: SaleOrderLine<MultipleIds> = env.search(&SearchType::Nothing)?;
    let first = found.into_iter().next().expect("three lines");
    first.get_siblings_total(&mut env)?;
    assert_eq!(cached(&env, "sale_order_line", "siblings_total", &lines), 1);
    Ok(())
}

/// A huge recordset is loaded in batches, not pulled into memory whole at the first read.
#[test]
fn test_prefetching_is_capped() -> Result<()> {
    let app = new_app();
    let ids = tags(&app, 1500)?;

    let mut env = app.new_env()?;
    let found: Tag<MultipleIds> = env.search(&SearchType::Nothing)?;
    let mut records = found.into_iter();
    records.next().expect("tags").get_name(&mut env)?;
    assert_eq!(cached(&env, "tag", "name", &ids), 1000);

    let last = records.last().expect("tags");
    last.get_name(&mut env)?;
    assert_eq!(
        cached(&env, "tag", "name", &ids),
        1500,
        "the next batch comes with the next record outside the first"
    );
    Ok(())
}

/// What is already in the cache, dirty included, is not replaced by what the database holds.
#[test]
fn test_prefetching_keeps_unsaved_changes() -> Result<()> {
    let app = new_app();
    let ids = tags(&app, 2)?;

    let mut env = app.new_env()?;
    let second: Tag<SingleId> = env.get_record(ids[1].into());
    second.set_name("renamed".to_string(), &mut env)?;

    let found: Tag<MultipleIds> = env.search(&SearchType::Nothing)?;
    let names: Vec<String> = found
        .into_iter()
        .map(|tag| Ok(tag.get_name(&mut env)?.clone()))
        .collect::<Result<_>>()?;
    assert_eq!(names, vec!["tag 0", "renamed"]);
    Ok(())
}
