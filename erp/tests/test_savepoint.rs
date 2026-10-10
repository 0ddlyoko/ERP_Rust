use erp::Result;
use erp::app::Application;
use erp_types::cache::{Dirty, Update};
use erp_types::field::SingleId;
use erp_types::model::MapOfFields;
use std::error::Error;
use std::fmt;
use test_utilities::models::{SaleOrder, SaleOrderLine, Tag};

#[derive(Debug, Clone)]
pub struct UselessError {}

impl fmt::Display for UselessError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Useless error")
    }
}

impl Error for UselessError {}

#[test]
fn test_savepoint_rollback() -> Result<()> {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.post_register();
    let mut env = app.new_env()?;

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    for (field, value) in map.fields {
        env.cache.insert_field_in_cache(
            "sale_order",
            &field,
            &[1],
            value,
            &Dirty::NotUpdateDirty,
            &Update::UpdateIfExists,
        );
    }

    let sale_order_line: SaleOrder<SingleId> = env.get_record(1.into());
    let _result: Result<()> = env.savepoint(|env| {
        // Update the record
        sale_order_line.set_name("1ddlyoko".to_string(), env)?;
        sale_order_line.set_total_price(420, env)?;

        // Check that it has been updated
        assert_eq!(sale_order_line.get_name(env)?, "1ddlyoko");
        assert_eq!(*sale_order_line.get_total_price(env)?, 420);

        // Throw a random error to rollback what we did here
        Err(erp::Error::Business(Box::new(UselessError {})))
    });

    // Check if it has not been committed
    assert_eq!(sale_order_line.get_name(&mut env)?, "0ddlyoko");
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 0);

    // Do it again, but here commit
    let _result: Result<()> = env.savepoint(|env| {
        // Update the record
        sale_order_line.set_name("1ddlyoko".to_string(), env)?;
        sale_order_line.set_total_price(420, env)?;

        // Check that it has been updated
        assert_eq!(sale_order_line.get_name(env)?, "1ddlyoko");
        assert_eq!(*sale_order_line.get_total_price(env)?, 420);

        Ok(())
    });

    // Check if it has not been committed
    assert_eq!(sale_order_line.get_name(&mut env)?, "1ddlyoko");
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 420);

    Ok(())
}

/// The in-memory database says which refusal it is, not only in words.
#[test]
fn test_the_in_memory_database_types_its_refusals() -> Result<()> {
    use erp::database::Database;
    use erp::database::cache::CacheDatabaseError;

    let app = Application::new_test();
    let mut database = app.create_new_database()?;
    let refusal = |error: Box<dyn Error + Send + Sync>| {
        error
            .downcast_ref::<CacheDatabaseError>()
            .cloned()
            .expect("a typed refusal")
    };

    assert_eq!(
        refusal(database.savepoint_commit("svp_a").unwrap_err()),
        CacheDatabaseError::MissingSavepoint {
            name: "svp_a".to_string(),
            operation: "commit",
        }
    );
    database.start_transaction()?;
    database.savepoint("svp_a")?;
    database.savepoint("svp_b")?;
    assert_eq!(
        refusal(database.savepoint_rollback("svp_a").unwrap_err()),
        CacheDatabaseError::NotTheLastSavepoint {
            name: "svp_a".to_string(),
        }
    );
    database.commit_transaction()?;
    assert_eq!(
        refusal(database.commit_transaction().unwrap_err()),
        CacheDatabaseError::NoTransaction {
            operation: "commit"
        }
    );
    Ok(())
}
