use erp::cache::Cache;
use erp::field::FieldType;
use erp::model::{MapOfFields, ModelManager};
use std::collections::HashMap;
use test_utilities::models::SaleOrder;

#[test]
fn test_get_and_insert_field() {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder>();
    let mut cache = Cache::new(&model_manager);
    let mut cached_fields = HashMap::new();
    cached_fields.insert(
        "my_field".to_string(),
        Some(FieldType::String("my_value".to_string())),
    );
    cache
        .get_cache_models_mut("sale_order")
        .get_model_or_create(1)
        .insert_fields(MapOfFields::new(cached_fields));

    // Check if retrieving the field is correct
    let cache_field = cache.get_record_field("sale_order", 1, "my_field");
    assert!(cache_field.unwrap().get().is_some());
    assert_eq!(
        cache_field.unwrap().get().unwrap(),
        &FieldType::String("my_value".to_string())
    );

    // Modify it
    cache.insert_record_field(
        "sale_order",
        "my_field",
        1,
        Some(FieldType::String("my_value_2".to_string())),
    );
    let cache_field = cache.get_record_field("sale_order", 1, "my_field");
    assert!(cache_field.is_some());
    assert!(cache_field.unwrap().get().is_some());
    assert_eq!(
        cache_field.unwrap().get().unwrap(),
        &FieldType::String("my_value_2".to_string())
    );

    // Clear the field
    cache.insert_record_field("sale_order", "my_field", 1, None);
    let cache_field = cache.get_record_field("sale_order", 1, "my_field");
    assert!(cache_field.is_some());
    assert!(cache_field.unwrap().get().is_none());
    // Put field back
    cache.insert_record_field(
        "sale_order",
        "my_field",
        1,
        Some(FieldType::String("my_value_2".to_string())),
    );

    // Insert another model
    cache
        .get_cache_models_mut("sale_order")
        .get_model_or_create(2);
    // Inserting another model shouldn't have modified the other field
    let cache_field = cache.get_record_field("sale_order", 1, "my_field");
    assert!(cache_field.is_some());
    assert!(cache_field.unwrap().get().is_some());
    assert_eq!(
        cache_field.unwrap().get().unwrap(),
        &FieldType::String("my_value_2".to_string())
    );

    // Modifying the other model shouldn't modify the other field
    cache.insert_record_field(
        "sale_order",
        "my_field",
        2,
        Some(FieldType::String("my_value_3".to_string())),
    );
    let cache_field = cache.get_record_field("sale_order", 1, "my_field");
    assert!(cache_field.is_some());
    assert!(cache_field.unwrap().get().is_some());
    assert_eq!(
        cache_field.unwrap().get().unwrap(),
        &FieldType::String("my_value_2".to_string())
    );
    let cache_field = cache.get_record_field("sale_order", 2, "my_field");
    assert!(cache_field.is_some());
    assert!(cache_field.unwrap().get().is_some());
    assert_eq!(
        cache_field.unwrap().get().unwrap(),
        &FieldType::String("my_value_3".to_string())
    );
}
