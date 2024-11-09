mod models;

use core::model::MapOfFields;
use core::environment::Environment;
use core::model::ModelManager;
use std::collections::HashMap;

use models::sale_order::SaleOrder;

#[test]
fn test_fill_default_values_on_map() {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder>();
    let env = Environment::new(&model_manager);

    let mut map: core::model::MapOfFields = core::model::MapOfFields::new(HashMap::new());
    env.fill_default_values_on_map("sale_order", &mut map);

    let name: Option<String> = map.get_option("name");
    let total_price: Option<i64> = map.get_option("total_price");

    assert!(name.is_some());
    assert!(total_price.is_some());

    let name = name.unwrap();
    let total_price = total_price.clone().unwrap();
    assert_eq!(name, "0ddlyoko".to_string());
    assert_eq!(total_price, 42);
}

#[test]
fn test_get_record() {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder>();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    let map = core::cache::Cache::transform_map_to_fields_into_cache(&map);
    env.cache.insert_record_model_with_map("sale_order", 1, map);
    env.cache.clear_all_dirty_of_model("sale_order", 1);

    // Get the record
    let sale_order = env.get_record::<SaleOrder>(1);
    assert!(sale_order.is_ok());
    let sale_order = sale_order.unwrap();
    assert!(sale_order.is_some());
    let mut sale_order = sale_order.unwrap();
    assert_eq!(sale_order.id, 1);
    assert_eq!(sale_order.name, "0ddlyoko");
    assert_eq!(sale_order.total_price, 42);
    let name_cache_record = env.cache.get_record_field("sale_order", 1, "name");
    let total_price_cache_record = env.cache.get_record_field("sale_order", 1, "total_price");
    assert!(name_cache_record.is_some());
    assert!(total_price_cache_record.is_some());
    let name_cache_record = name_cache_record.unwrap();
    let total_price_cache_record = total_price_cache_record.unwrap();
    assert!(!name_cache_record.is_dirty());
    assert!(!total_price_cache_record.is_dirty());
    assert!(name_cache_record.is_set());
    assert!(total_price_cache_record.is_set());
    assert!(name_cache_record.get().is_some());
    assert_eq!(*name_cache_record.get().unwrap(), core::cache::CacheFieldValue::String("0ddlyoko".to_string()));
    assert_eq!(*total_price_cache_record.get().unwrap(), core::cache::CacheFieldValue::Int(42));

    // Changing the price should not alter the cache (as it's not already saved)
    sale_order.total_price = 50;
    assert!(!name_cache_record.is_dirty());
    assert!(!total_price_cache_record.is_dirty());
    assert!(name_cache_record.is_set());
    assert!(total_price_cache_record.is_set());
    assert!(name_cache_record.get().is_some());
    assert_eq!(*name_cache_record.get().unwrap(), core::cache::CacheFieldValue::String("0ddlyoko".to_string()));
    assert_eq!(*total_price_cache_record.get().unwrap(), core::cache::CacheFieldValue::Int(42));

    // But saving it should
    env.save_record(&sale_order);

    let name_cache_record = env.cache.get_record_field("sale_order", 1, "name");
    let total_price_cache_record = env.cache.get_record_field("sale_order", 1, "total_price");
    assert!(name_cache_record.is_some());
    assert!(total_price_cache_record.is_some());
    let name_cache_record = name_cache_record.unwrap();
    let total_price_cache_record = total_price_cache_record.unwrap();
    assert!(!name_cache_record.is_dirty());
    assert!(total_price_cache_record.is_dirty());
    assert!(name_cache_record.is_set());
    assert!(total_price_cache_record.is_set());
    assert!(name_cache_record.get().is_some());
    assert_eq!(*name_cache_record.get().unwrap(), core::cache::CacheFieldValue::String("0ddlyoko".to_string()));
    assert_eq!(*total_price_cache_record.get().unwrap(), core::cache::CacheFieldValue::Int(50));

    let cache_model = env.cache.get_cache_record("sale_order", 1);
    assert!(cache_model.is_some());
    let dirty_fields = cache_model.unwrap().get_fields_dirty();
    assert_eq!(dirty_fields.len(), 1);
    assert!(!dirty_fields.contains_key("name"));
    assert!(dirty_fields.contains_key("total_price"));
}
