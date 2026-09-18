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

fn create_line(env: &mut erp::environment::Environment, amount: i32) -> Result<()> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("amount", amount);
    let _line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
    Ok(())
}

fn count_lines(env: &mut erp::environment::Environment, amount: i32) -> Result<usize> {
    let found: SaleOrderLine<MultipleIds> = env.search(&make_domain!([("amount", "=", amount)]))?;
    Ok(found.id.get_ids_ref().len())
}

/// Several environments must be able to exist at the same time on one application.
#[test]
fn test_several_environments_coexist() -> Result<()> {
    let app = new_app();

    let mut first = app.new_env()?;
    let mut second = app.new_env()?;
    let mut third = app.new_env()?;

    create_line(&mut first, 1)?;
    create_line(&mut second, 2)?;
    create_line(&mut third, 3)?;

    first.close()?;
    second.close()?;
    third.close()?;

    let mut env = app.new_env()?;
    for amount in 1..=3 {
        assert_eq!(
            count_lines(&mut env, amount)?,
            1,
            "amount {amount} must be committed"
        );
    }
    Ok(())
}

/// An open environment must not observe writes another one has not committed yet.
#[test]
fn test_environments_are_isolated_until_commit() -> Result<()> {
    let app = new_app();

    let mut writer = app.new_env()?;
    let mut reader = app.new_env()?;

    create_line(&mut writer, 42)?;
    assert_eq!(
        count_lines(&mut reader, 42)?,
        0,
        "an environment must not see another's uncommitted writes"
    );

    writer.close()?;
    assert_eq!(
        count_lines(&mut reader, 42)?,
        0,
        "an already-open transaction keeps the snapshot it started from"
    );
    drop(reader);

    let mut fresh = app.new_env()?;
    assert_eq!(
        count_lines(&mut fresh, 42)?,
        1,
        "an environment opened after the commit must see the row"
    );
    Ok(())
}

/// A rolled back environment must leave nothing behind for the others.
#[test]
fn test_rollback_does_not_leak_to_other_environments() -> Result<()> {
    let app = new_app();

    let mut kept = app.new_env()?;
    let mut discarded = app.new_env()?;

    create_line(&mut kept, 7)?;
    create_line(&mut discarded, 9)?;

    kept.close()?;
    drop(discarded);

    let mut env = app.new_env()?;
    assert_eq!(
        count_lines(&mut env, 7)?,
        1,
        "the committed row must survive"
    );
    assert_eq!(
        count_lines(&mut env, 9)?,
        0,
        "the dropped environment must roll back"
    );
    Ok(())
}

/// The real target: concurrent requests, each on its own thread and its own transaction.
#[test]
fn test_environments_commit_from_parallel_threads() -> Result<()> {
    let app = new_app();
    let thread_count = 8;

    // Shared by reference: this only compiles if `Application` is `Sync`.
    let shared = &app;
    std::thread::scope(|scope| {
        for amount in 0..thread_count {
            scope.spawn(move || {
                let mut env = shared.new_env().expect("environment");
                create_line(&mut env, amount).expect("create");
                env.close().expect("close");
            });
        }
    });

    let mut env = app.new_env()?;
    for amount in 0..thread_count {
        assert_eq!(
            count_lines(&mut env, amount)?,
            1,
            "the row written by the thread handling amount {amount} must be committed"
        );
    }
    Ok(())
}
