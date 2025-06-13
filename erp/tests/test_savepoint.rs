use erp::app::Application;
use erp::cache::{Dirty, Update};
use erp::field::SingleId;
use erp::model::MapOfFields;
use std::error::Error;
use std::fmt;
use test_utilities::models::{SaleOrder, SaleOrderLine};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

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
    app.model_manager.post_register();
    let mut env = app.new_env();

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    env.cache.insert_fields_in_cache("sale_order", 1, map, &Dirty::NotUpdateDirty, &Update::UpdateIfExists);

    let sale_order_line: SaleOrder<SingleId> = env.get_record(1.into());
    let _result: Result<()> = env.savepoint(|env| {
        // Update the record
        sale_order_line.set_name("1ddlyoko".to_string(), env)?;
        sale_order_line.set_total_price(420, env)?;

        // Check that it has been updated
        assert_eq!(sale_order_line.get_name(env)?, "1ddlyoko");
        assert_eq!(*sale_order_line.get_total_price(env)?, 420);

        // Throw a random error to rollback what we did here
        Err(Box::new(UselessError {}))
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
