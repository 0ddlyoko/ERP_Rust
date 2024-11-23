mod models;

use erp::field::FieldType;
use erp::model::{ MapOfFields, ModelManager };
use erp::environment::Environment;
use std::collections::HashMap;

use models::sale_order::SaleOrder;

#[test]
fn test_fill_default_values_on_map() {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder>();
    let env = Environment::new(&model_manager);

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    env.fill_default_values_on_map("sale_order", &mut map);

    let name: Option<String> = map.get_option("name");
    let price: Option<i64> = map.get_option("price");

    assert!(name.is_some());
    assert!(price.is_some());

    let name = name.unwrap();
    let price = price.unwrap();
    assert_eq!(name, "0ddlyoko".to_string());
    assert_eq!(price, 42);
}

#[test]
fn test_get_record() {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder>();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    env.cache.insert_record_model_with_map("sale_order", 1, map);
    env.cache.clear_all_dirty_of_model("sale_order", 1);

    // Get the record
    let sale_order = env.get_record::<SaleOrder>(1);
    assert!(sale_order.is_ok());
    let mut sale_order = sale_order.unwrap();
    assert_eq!(sale_order.id, 1);
    assert_eq!(sale_order.name, "0ddlyoko");
    assert_eq!(sale_order.price, 42);
    assert_eq!(sale_order.amount, 10);
    assert_eq!(sale_order.total_price, 0, "Should be 0 as \"insert_record_model_with_map\" does not call computed methods");
    let name_cache_record = env.cache.get_record_field("sale_order", 1, "name");
    let price_cache_record = env.cache.get_record_field("sale_order", 1, "price");
    assert!(name_cache_record.is_some());
    assert!(price_cache_record.is_some());
    let name_cache_record = name_cache_record.unwrap();
    let price_cache_record = price_cache_record.unwrap();
    assert!(!name_cache_record.is_dirty());
    assert!(!price_cache_record.is_dirty());
    assert!(name_cache_record.is_set());
    assert!(price_cache_record.is_set());
    assert!(name_cache_record.get().is_some());
    assert_eq!(*name_cache_record.get().unwrap(), FieldType::String("0ddlyoko".to_string()));
    assert_eq!(*price_cache_record.get().unwrap(), FieldType::Integer(42));

    // Changing the price should not alter the cache (as it's not already saved)
    sale_order.price = 50;
    assert!(!name_cache_record.is_dirty());
    assert!(!price_cache_record.is_dirty());
    assert!(name_cache_record.is_set());
    assert!(price_cache_record.is_set());
    assert!(name_cache_record.get().is_some());
    assert_eq!(*name_cache_record.get().unwrap(), FieldType::String("0ddlyoko".to_string()));
    assert_eq!(*price_cache_record.get().unwrap(), FieldType::Integer(42));

    // But saving it should
    env.save_record(&sale_order);

    let name_cache_record = env.cache.get_record_field("sale_order", 1, "name");
    let price_cache_record = env.cache.get_record_field("sale_order", 1, "price");
    assert!(name_cache_record.is_some());
    assert!(price_cache_record.is_some());
    let name_cache_record = name_cache_record.unwrap();
    let price_cache_record = price_cache_record.unwrap();
    assert!(!name_cache_record.is_dirty());
    assert!(price_cache_record.is_dirty());
    assert!(name_cache_record.is_set());
    assert!(price_cache_record.is_set());
    assert!(name_cache_record.get().is_some());
    assert_eq!(*name_cache_record.get().unwrap(), FieldType::String("0ddlyoko".to_string()));
    assert_eq!(*price_cache_record.get().unwrap(), FieldType::Integer(50));

    let cache_model = env.cache.get_cache_record("sale_order", 1);
    assert!(cache_model.is_some());
    let dirty_fields = cache_model.unwrap().get_fields_dirty();
    assert_eq!(dirty_fields.len(), 1);
    assert!(!dirty_fields.contains_key("name"));
    assert!(dirty_fields.contains_key("price"));
}

#[test]
fn test_get_record_from_xxx() {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder>();
    let internal_model = model_manager.get_model("sale_order").unwrap().first();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    env.cache.insert_record_model_with_map("sale_order", 1, map);
    env.cache.clear_all_dirty_of_model("sale_order", 1);

    // Get the record
    let sale_order = env.get_record::<SaleOrder>(1);
    let sale_order_by_name = env.get_record_from_name("sale_order", 1);
    let sale_order_by_internal_model = env.get_record_from_internal_model(internal_model, 1);
    let sale_order_by_unknown_name = env.get_record_from_name("sale_order_unknown", 1);

    assert!(sale_order.is_ok());
    let sale_order = sale_order.unwrap();
    assert_eq!(sale_order.get_id(), 1);
    assert_eq!(sale_order.name, "0ddlyoko");
    assert_eq!(sale_order.price, 42);

    assert!(sale_order_by_name.is_ok());
    let sale_order_by_name = sale_order_by_name.unwrap();
    assert_eq!(sale_order_by_name.get_id(), 1);

    assert!(sale_order_by_internal_model.is_ok());
    let sale_order_by_internal_model = sale_order_by_internal_model.unwrap();
    assert_eq!(sale_order_by_internal_model.get_id(), 1);

    assert!(sale_order_by_unknown_name.is_err());
}

#[test]
fn test_compute_method() {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder>();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    env.cache.insert_record_model_with_map("sale_order", 1, map);
    env.cache.clear_all_dirty_of_model("sale_order", 1);

    // Get the record
    let sale_order = env.get_record::<SaleOrder>(1);
    assert!(sale_order.is_ok());
    let sale_order = sale_order.unwrap();
    assert_eq!(sale_order.id, 1);
    assert_eq!(sale_order.name, "0ddlyoko");
    assert_eq!(sale_order.price, 42);
    assert_eq!(sale_order.amount, 10);
    assert_eq!(sale_order.total_price, 0, "Should be 0 as \"insert_record_model_with_map\" does not call computed methods");

    // Call the computed method
    env.call_compute_fields("sale_order", 1, &["total_price".to_string()]).expect("Computed field failed");

    let sale_order = env.get_record::<SaleOrder>(1);
    assert!(sale_order.is_ok());
    let sale_order = sale_order.unwrap();
    assert_eq!(sale_order.id, 1);
    assert_eq!(sale_order.name, "0ddlyoko");
    assert_eq!(sale_order.price, 42);
    assert_eq!(sale_order.amount, 10);
    assert_eq!(sale_order.total_price, 420);
}
