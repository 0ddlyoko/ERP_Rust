use erp::environment::Environment;
use erp::field::{FieldType, SingleId};
use erp::model::{MapOfFields, ModelManager};
use std::collections::HashMap;
use std::error::Error;
use test_utilities::models::SaleOrder;

#[test]
fn test_fill_default_values_on_map() {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder<_>>();
    let env = Environment::new(&model_manager);

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    env.fill_default_values_on_map("sale_order", &mut map);

    let name: Option<&String> = map.get_option("name");
    let price: Option<&i32> = map.get_option("price");

    assert!(name.is_some());
    assert!(price.is_some());

    let name = name.unwrap();
    let price = price.unwrap();
    assert_eq!(name.clone(), "0ddlyoko".to_string());
    assert_eq!(*price, 42);
}

#[test]
fn test_get_record() -> Result<(), Box<dyn Error>> {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder<_>>();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    env.cache.insert_record_model_with_map("sale_order", 1, map);
    env.cache.clear_dirty("sale_order", &1);

    // Get the record
    let sale_order = env.get_record::<SaleOrder<_>, SingleId>(1.into());
    assert_eq!(sale_order.id, 1);
    assert_eq!(sale_order.get_name(&mut env)?, "0ddlyoko");
    assert_eq!(*sale_order.get_price(&mut env)?, 42);
    assert_eq!(*sale_order.get_amount(&mut env)?, 10);
    assert_eq!(
        *sale_order.get_total_price(&mut env)?, 0,
        "Should be 0 as \"insert_record_model_with_map\" does not call computed methods"
    );
    let name_cache_record = env.cache.get_record_field("sale_order", &1, "name");
    let price_cache_record = env.cache.get_record_field("sale_order", &1, "price");
    assert!(name_cache_record.is_some());
    assert!(price_cache_record.is_some());
    let name_cache_record = name_cache_record.unwrap();
    let price_cache_record = price_cache_record.unwrap();
    assert!(name_cache_record.is_set());
    assert!(price_cache_record.is_set());
    assert!(name_cache_record.get().is_some());
    assert_eq!(
        *name_cache_record.get().unwrap(),
        FieldType::String("0ddlyoko".to_string())
    );
    assert_eq!(*price_cache_record.get().unwrap(), FieldType::Integer(42));
    // Dirty
    let dirty_fields = env.cache.get_cache_models("sale_order").get_dirty(&1);
    assert!(dirty_fields.is_none());

    // Changing the price should alter the cache
    sale_order.set_price(50, &mut env)?;

    let name_cache_record = env.cache.get_record_field("sale_order", &1, "name");
    let price_cache_record = env.cache.get_record_field("sale_order", &1, "price");
    assert!(name_cache_record.is_some());
    assert!(price_cache_record.is_some());
    let name_cache_record = name_cache_record.unwrap();
    let price_cache_record = price_cache_record.unwrap();
    assert!(name_cache_record.is_set());
    assert!(price_cache_record.is_set());
    assert!(name_cache_record.get().is_some());
    assert_eq!(
        *name_cache_record.get().unwrap(),
        FieldType::String("0ddlyoko".to_string())
    );
    assert_eq!(*price_cache_record.get().unwrap(), FieldType::Integer(50));
    // Price has been modified, it should be dirty
    let dirty_fields = env.cache.get_cache_models("sale_order").get_dirty(&1);
    assert!(dirty_fields.is_some());
    let dirty_fields = dirty_fields.unwrap();
    assert_eq!(dirty_fields.len(), 1);
    assert!(dirty_fields.contains(&"price".to_string()));
    let cache_models = env.cache.get_cache_models_mut("sale_order");
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
    let internal_model = model_manager.get_model("sale_order").unwrap().first();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    env.cache.insert_record_model_with_map("sale_order", 1, map);
    env.cache.clear_dirty("sale_order", &1);

    // Get the record
    let sale_order = env.get_record::<SaleOrder<_>, _>(1.into());
    let sale_order_by_name = env.get_record_from_name::<SingleId>("sale_order", 1.into())?;
    let sale_order_by_internal_model = env.get_record_from_internal_model::<SingleId>(internal_model, 1.into())?;
    let sale_order_by_unknown_name = env.get_record_from_name::<SingleId>("sale_order_unknown", 1.into());

    assert_eq!(sale_order.get_id(), 1);
    assert_eq!(sale_order.get_name(&mut env)?, "0ddlyoko");
    assert_eq!(*sale_order.get_price(&mut env)?, 42);

    assert_eq!(sale_order_by_name.get_id_mode().get_id(), 1);

    assert_eq!(sale_order_by_internal_model.get_id_mode().get_id(), 1);

    assert!(sale_order_by_unknown_name.is_err());
    Ok(())
}

#[test]
fn test_compute_method() -> Result<(), Box<dyn Error>> {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder<_>>();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    env.cache.insert_record_model_with_map("sale_order", 1, map);
    env.cache.clear_dirty("sale_order", &1);

    // Get the record
    let sale_order: SaleOrder<SingleId> = env.get_record(1.into());
    assert_eq!(sale_order.id, 1);
    assert_eq!(sale_order.get_name(&mut env)?, "0ddlyoko");
    assert_eq!(*sale_order.get_price(&mut env)?, 42);
    assert_eq!(*sale_order.get_amount(&mut env)?, 10);
    assert_eq!(*sale_order.get_total_price(&mut env)?, 0,
        "Should be 0 as \"insert_record_model_with_map\" does not call computed methods"
    );

    // Call the computed method
    // TODO Bring back computed fields
    // env.call_compute_fields("sale_order", &1.into(), &["total_price".to_string()])
    //     .expect("Computed field should not fail");
    //
    // let sale_order = env.get_record::<SaleOrder<_>, _>(1.into());
    // assert_eq!(sale_order.id, 1);
    // assert_eq!(sale_order.get_name(&mut env)?, "0ddlyoko");
    // assert_eq!(*sale_order.get_price(&mut env)?, 42);
    // assert_eq!(*sale_order.get_amount(&mut env)?, 10);
    // assert_eq!(*sale_order.get_total_price(&mut env)?, 420);
    Ok(())
}
