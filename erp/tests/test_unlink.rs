use erp::app::Application;
use erp::database::Database;
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

fn create_order(
    env: &mut erp::environment::Environment,
    name: &str,
) -> Result<SaleOrder<SingleId>> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", name);
    env.create_new_record_from_map(map)
}

fn create_line(
    env: &mut erp::environment::Environment,
    order: &SaleOrder<SingleId>,
    amount: i32,
) -> Result<SaleOrderLine<SingleId>> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("amount", amount);
    map.insert("order", order.get_id());
    env.create_new_record_from_map(map)
}

/// A committed deletion must not come back when the next transaction takes its snapshot.
#[test]
fn test_deletion_survives_a_commit() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let order = create_order(&mut env, "doomed")?;
    let ids = order.id.clone();
    env.close()?;

    let mut env = app.new_env()?;
    assert_eq!(env.unlink("sale_order", &ids)?, 1);
    env.close()?;

    let mut env = app.new_env()?;
    let found: SaleOrder<MultipleIds> = env.search(&make_domain!([("name", "=", "doomed")]))?;
    assert!(found.id.is_empty(), "the deleted record must stay deleted");
    Ok(())
}

/// Deleting inside a transaction that is rolled back must delete nothing.
#[test]
fn test_deletion_is_rolled_back_with_the_transaction() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let order = create_order(&mut env, "spared")?;
    let ids = order.id.clone();
    env.close()?;

    let mut env = app.new_env()?;
    assert_eq!(env.unlink("sale_order", &ids)?, 1);
    drop(env);

    let mut env = app.new_env()?;
    let found: SaleOrder<MultipleIds> = env.search(&make_domain!([("name", "=", "spared")]))?;
    assert_eq!(
        found.id.get_ids_ref().len(),
        1,
        "a rollback must undo the delete"
    );
    Ok(())
}

/// Deleting a parent leaves its children in place, with their foreign key cleared.
#[test]
fn test_children_are_detached_not_deleted() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let order = create_order(&mut env, "parent")?;
    let line = create_line(&mut env, &order, 11)?;
    let line_ids = line.id.clone();
    env.close()?;

    let mut env = app.new_env()?;
    env.unlink("sale_order", &order.id)?;
    env.close()?;

    let mut env = app.new_env()?;
    let surviving: SaleOrderLine<MultipleIds> = env.search(&make_domain!([("amount", "=", 11)]))?;
    assert_eq!(
        surviving.id.get_ids_ref(),
        line_ids.as_ref(),
        "the child must survive its parent"
    );
    let rows = env.read("sale_order_line", &line_ids, &["order"])?;
    assert!(
        rows[0].get_option::<&u32>("order").is_none(),
        "the child's foreign key must be cleared, got {:?}",
        rows[0]
    );
    Ok(())
}

/// A search that walks a relation must not surface a record that was deleted.
#[test]
fn test_dotted_path_search_ignores_deleted_records() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let kept = create_order(&mut env, "kept")?;
    let removed = create_order(&mut env, "removed")?;
    create_line(&mut env, &kept, 5)?;
    create_line(&mut env, &removed, 5)?;
    env.close()?;

    let mut env = app.new_env()?;
    let before: SaleOrder<MultipleIds> = env.search(&make_domain!([("lines.amount", "=", 5)]))?;
    assert_eq!(before.id.get_ids_ref().len(), 2);

    env.unlink("sale_order", &removed.id)?;
    let after: SaleOrder<MultipleIds> = env.search(&make_domain!([("lines.amount", "=", 5)]))?;
    assert_eq!(
        after.id.get_ids_ref(),
        kept.id.as_ref(),
        "a deleted order must not be reachable through its lines"
    );
    Ok(())
}

/// A record deleted while it still had a pending recomputation must not be revived by the
/// compute engine.
#[test]
fn test_deleted_record_is_not_resurrected_by_compute() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let order = create_order(&mut env, "recompute")?;
    let line = create_line(&mut env, &order, 3)?;
    env.close()?;

    let mut env = app.new_env()?;
    // Dirties the line and flags the order's total_price for recomputation.
    line.set_amount(99, &mut env)?;
    env.unlink("sale_order_line", &line.id)?;
    env.save_all_to_db()?;
    env.close()?;

    let mut env = app.new_env()?;
    let found: SaleOrderLine<MultipleIds> = env.search(&make_domain!([("amount", "=", 99)]))?;
    assert!(found.id.is_empty(), "the deleted line must not reappear");
    let still_there: SaleOrderLine<MultipleIds> =
        env.search(&make_domain!([("amount", "=", 3)]))?;
    assert!(still_there.id.is_empty(), "nor under its previous value");
    Ok(())
}

/// Deleting reports how many rows actually went, and ignores ids that were never there.
#[test]
fn test_delete_counts_only_what_existed() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let order = create_order(&mut env, "counted")?;
    env.close()?;

    let mut env = app.new_env()?;
    let mut ids: MultipleIds = order.id.clone().into();
    ids += MultipleIds::from(vec![9999]);
    assert_eq!(
        env.unlink("sale_order", &ids)?,
        1,
        "only the row that existed counts"
    );
    Ok(())
}

/// Deleting nothing is not an error.
#[test]
fn test_delete_empty_set_is_a_noop() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    assert_eq!(
        env.unlink("sale_order", &MultipleIds::from(Vec::<u32>::new()))?,
        0
    );
    Ok(())
}

/// Rows removed without going through `unlink` leave dangling foreign keys behind. A relational
/// search must neither trip over them nor surface the record they point at.
///
/// `unlink` detaches children first, so this is only reachable by deleting at the backend level —
/// which is exactly what a future `on_delete` policy other than set-null would do.
#[test]
fn test_relational_search_tolerates_dangling_references() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let kept = create_order(&mut env, "kept")?;
    let removed = create_order(&mut env, "removed")?;
    create_line(&mut env, &kept, 7)?;
    create_line(&mut env, &removed, 7)?;
    env.close()?;

    let mut env = app.new_env()?;
    // Straight to the backend: the lines keep pointing at an order that no longer exists.
    env.database.delete("sale_order", removed.id.as_ref())?;

    let found: SaleOrder<MultipleIds> = env.search(&make_domain!([("lines.amount", "=", 7)]))?;
    assert_eq!(
        found.id.get_ids_ref(),
        kept.id.as_ref(),
        "a dangling reference must not resurrect the order it points at"
    );
    Ok(())
}

/// Deleting a record recomputes what depended on it.
///
/// `unlink` clears the record's relational fields through the normal write path, which is what
/// flags the dependents, and flushes before the row goes.
#[test]
fn test_deleting_a_line_recomputes_the_order_total() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let order = create_order(&mut env, "total")?;
    let mut line_ids = vec![];
    for (price, amount) in [(2, 3), (4, 5)] {
        let mut map: MapOfFields = MapOfFields::new(HashMap::new());
        map.insert("price", price);
        map.insert("amount", amount);
        map.insert("order", order.get_id());
        let line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
        line_ids.push(line.id.clone());
    }
    assert_eq!(*order.get_total_price(&mut env)?, 26, "2*3 + 4*5");
    env.close()?;

    let mut env = app.new_env()?;
    env.unlink("sale_order_line", &line_ids[1])?;
    assert_eq!(
        *order.get_total_price(&mut env)?,
        6,
        "the total must drop to the surviving line"
    );
    env.close()?;

    // And the recomputed total must have been persisted, not just held in cache.
    let mut env = app.new_env()?;
    let reloaded: SaleOrder<MultipleIds> = env.search(&make_domain!([("name", "=", "total")]))?;
    assert_eq!(reloaded.get_total_price(&mut env)?, vec![&6]);
    Ok(())
}
