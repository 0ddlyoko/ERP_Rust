//! The ORM's verbs, reached from the model rather than from the environment.
//!
//! `SaleOrder::search(...)` says what it acts on before what it does, which is the order the
//! caller is thinking in. It is the same call underneath.

use erp::app::Application;
use erp_search::SearchOptions;
use erp_search_code_gen::make_domain;
use erp_types::field::{MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use test_utilities::TestLibPlugin;
use test_utilities::models::SaleOrder;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;
    Ok(app)
}

fn values(name: &str) -> MapOfFields {
    let mut map = MapOfFields::new(HashMap::new());
    map.insert("name", name);
    map
}

#[test]
fn test_create_and_search_from_the_model() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    let created =
        SaleOrder::<MultipleIds>::create(vec![values("first"), values("second")], &mut env)?;
    assert_eq!(created.get_ids_ref().len(), 2);

    let found =
        SaleOrder::<MultipleIds>::search(&make_domain!([("name", "=", "first")]), &mut env)?;
    assert_eq!(found.get_ids_ref().len(), 1);
    assert_eq!(found.get_name(&mut env)?, vec![&"first".to_string()]);
    Ok(())
}

#[test]
fn test_count_and_search_options_from_the_model() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    SaleOrder::<MultipleIds>::create(vec![values("a"), values("b"), values("c")], &mut env)?;

    let all = make_domain!([]);
    assert_eq!(SaleOrder::<MultipleIds>::count(&all, &mut env)?, 3);

    let page =
        SaleOrder::<MultipleIds>::search_with(&all, &SearchOptions::new().with_limit(2), &mut env)?;
    assert_eq!(
        page.get_ids_ref().len(),
        2,
        "count ignores the limit, search does not"
    );
    Ok(())
}

#[test]
fn test_read_write_and_unlink_from_the_model() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    let orders = SaleOrder::<MultipleIds>::create(vec![values("before")], &mut env)?;

    orders.write(values("after"), &mut env)?;
    let rows = orders.read(&["name"], &mut env)?;
    assert_eq!(rows[0].get::<&String>("name"), &"after".to_string());

    assert_eq!(orders.delete(&mut env)?, 1);
    assert_eq!(
        SaleOrder::<MultipleIds>::count(&make_domain!([]), &mut env)?,
        0
    );
    Ok(())
}

/// `from_ids` builds a recordset over ids already known, without asking the database.
#[test]
fn test_browse_from_the_model() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    let created = SaleOrder::<MultipleIds>::create(vec![values("known")], &mut env)?;
    let id = created.get_ids_ref()[0];

    let one = SaleOrder::<SingleId>::from_id(id, &env);
    assert_eq!(one.get_id(), id);
    assert_eq!(one.get_name(&mut env)?, &"known".to_string());

    let many = SaleOrder::<MultipleIds>::from_ids(vec![id], &env);
    assert_eq!(many.get_ids_ref(), &vec![id]);
    Ok(())
}

/// A single record creates, reads and writes on its own too.
#[test]
fn test_the_single_record_verbs() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;

    let order = SaleOrder::<SingleId>::create(values("alone"), &mut env)?;
    order.write(values("renamed"), &mut env)?;
    assert_eq!(
        order.read(&["name"], &mut env)?[0].get::<&String>("name"),
        &"renamed".to_string()
    );
    assert_eq!(order.delete(&mut env)?, 1);
    Ok(())
}

/// The model-level verb and the environment-level one are the same call.
#[test]
fn test_both_spellings_agree() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    SaleOrder::<MultipleIds>::create(vec![values("same")], &mut env)?;

    let domain = make_domain!([("name", "=", "same")]);
    let from_model = SaleOrder::<MultipleIds>::search(&domain, &mut env)?;
    let from_env: SaleOrder<MultipleIds> = env.search(&domain)?;
    assert_eq!(from_model.get_ids_ref(), from_env.get_ids_ref());
    Ok(())
}
