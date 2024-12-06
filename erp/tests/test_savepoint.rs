use erp::environment::{Environment, Savepoint};
use erp::model::{MapOfFields, ModelManager};

use test_utilities::models::sale_order::SaleOrder;

#[test]
fn test_savepoint_rollback() {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder>();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    env.cache.insert_record_model_with_map("sale_order", 1, map);
    env.cache.clear_dirty("sale_order", 1);

    let savepoint = Savepoint::new(&env);

    // Update the record
    let mut sale_order = env.get_record::<SaleOrder>(1).unwrap();
    sale_order.name = "1ddlyoko".to_string();
    sale_order.price = 420;
    env.save_record(&sale_order);

    // Check that it has been updated
    let sale_order = env.get_record::<SaleOrder>(1).unwrap();
    assert_eq!(sale_order.name, "1ddlyoko");
    assert_eq!(sale_order.price, 420);

    // Rollback
    savepoint.rollback(&mut env);

    let sale_order = env.get_record::<SaleOrder>(1).unwrap();
    assert_eq!(sale_order.name, "0ddlyoko");
    assert_eq!(sale_order.price, 42);
}
