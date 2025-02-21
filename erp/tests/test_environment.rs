use erp::environment::Environment;
use erp::field::{FieldType, SingleId};
use erp::model::{MapOfFields, ModelManager};
use std::collections::HashMap;
use std::error::Error;
use test_utilities::models::{SaleOrder, SaleOrderLine};

#[test]
fn test_fill_default_values_on_map() {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder<_>>();
    let env = Environment::new(&model_manager);

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    env.fill_default_values_on_map("sale_order", &mut map);

    let name: Option<&String> = map.get_option("name");
    let total_price: Option<&i32> = map.get_option("total_price");

    assert!(name.is_some());
    assert!(total_price.is_some());

    let name = name.unwrap();
    let price = total_price.unwrap();
    assert_eq!(name.clone(), "0ddlyoko".to_string());
    assert_eq!(*price, 0);
}

#[test]
fn test_get_record() -> Result<(), Box<dyn Error>> {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrderLine<_>>();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order_line", &mut map);

    env.cache.insert_record_fields("sale_order_line", 1, map, false);

    // Get the record
    let sale_order_line = env.get_record::<SaleOrderLine<_>, SingleId>(1.into());
    assert_eq!(sale_order_line.id, 1);
    assert_eq!(*sale_order_line.get_price(&mut env)?, 42);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 10);
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 42 * 10, "Should not be 0 as the computed method is called");
    let price_cache_record = env.cache.get_record_field("sale_order_line", "price", &1);
    let amount_cache_record = env.cache.get_record_field("sale_order_line", "amount", &1);
    assert!(price_cache_record.is_some());
    assert!(amount_cache_record.is_some());
    let price_cache_record = price_cache_record.unwrap();
    let amount_cache_record = amount_cache_record.unwrap();
    assert_eq!(*price_cache_record, FieldType::Integer(42));
    assert_eq!(*amount_cache_record, FieldType::Integer(10));
    // Dirty
    let dirty_fields = env.cache.get_cache_models("sale_order_line").get_dirty(&1);
    assert!(dirty_fields.is_none());

    // Changing the price should alter the cache
    sale_order_line.set_price(50, &mut env)?;

    let price_cache_record = env.cache.get_record_field("sale_order_line", "amount", &1);
    let amount_cache_record = env.cache.get_record_field("sale_order_line", "name", &1);
    assert!(price_cache_record.is_some());
    assert!(amount_cache_record.is_some());
    let price_cache_record = price_cache_record.unwrap();
    let amount_cache_record = amount_cache_record.unwrap();
    assert_eq!(*price_cache_record, FieldType::Integer(50));
    assert_eq!(*amount_cache_record, FieldType::Integer(10));
    // Price has been modified, it should be dirty
    let dirty_fields = env.cache.get_cache_models("sale_order_line").get_dirty(&1);
    assert!(dirty_fields.is_some());
    let dirty_fields = dirty_fields.unwrap();
    assert_eq!(dirty_fields.len(), 1);
    assert!(dirty_fields.contains(&"price".to_string()));
    let cache_models = env.cache.get_cache_models_mut("sale_order_line");
    assert!(cache_models.get_model(&1).is_some());
    let dirty_fields = cache_models.get_dirty(&1);
    assert!(dirty_fields.is_some());
    assert!(dirty_fields
        .unwrap()
        .iter()
        .eq(["price".to_string()].iter()));

    // Clear dirty
    cache_models.clear_dirty(&1);
    assert!(cache_models.get_dirty(&1).is_none());

    Ok(())
}

#[test]
fn test_get_record_from_xxx() -> Result<(), Box<dyn Error>> {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder<_>>();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    env.cache.insert_record_fields("sale_order", 1, map, false);

    // Get the record
    let sale_order = env.get_record::<SaleOrder<_>, _>(1.into());

    assert_eq!(sale_order.get_id(), 1);
    assert_eq!(sale_order.get_name(&mut env)?, "0ddlyoko");
    Ok(())
}

#[test]
fn test_compute_method() -> Result<(), Box<dyn Error>> {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrderLine<_>>();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order_line", &mut map);

    env.cache.insert_record_fields("sale_order_line", 1, map, false);

    // Get the record
    let sale_order_line: SaleOrderLine<SingleId> = env.get_record(1.into());
    assert_eq!(sale_order_line.id, 1);
    assert_eq!(*sale_order_line.get_price(&mut env)?, 42);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 10);
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 42 * 10);

    // Modifying a key should call the computed method
    sale_order_line.set_price(50, &mut env)?;

    let sale_order_line = env.get_record::<SaleOrderLine<SingleId>, _>(1.into());
    assert_eq!(sale_order_line.id, 1);
    assert_eq!(*sale_order_line.get_price(&mut env)?, 50);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 10);
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 50 * 100);
    Ok(())
}
