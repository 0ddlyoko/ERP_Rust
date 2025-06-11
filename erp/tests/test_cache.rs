use erp::app::Application;
use erp::cache::Dirty;
use erp::cache::{Cache, Compute, Update};
use erp::field::{FieldType, IdMode, SingleId};
use erp::model::{MapOfFields, ModelManager};
use std::collections::HashMap;
use std::error::Error;
use test_utilities::models::{SaleOrder, SaleOrderLine};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[test]
fn test_get_and_insert_field() {
    let mut model_manager = ModelManager::default();
    model_manager.register_model::<SaleOrder<_>>();
    let mut cache = Cache::new(&model_manager);
    let id_1: SingleId = 1.into();
    let id_2: SingleId = 2.into();
    let mut cached_fields = HashMap::new();
    cached_fields.insert(
        "my_field".to_string(),
        Some(FieldType::String("my_value".to_string())),
    );
    cache
        .get_cache_models_mut("sale_order")
        .get_model_or_create(id_1.get_id())
        .insert_fields(MapOfFields::new(cached_fields), &Update::UpdateIfExists);

    // Check if retrieving the field is correct
    let cache_field = cache.get_field_from_cache("sale_order", "my_field", &id_1.get_id());
    assert!(cache_field.is_some());
    assert_eq!(cache_field.unwrap(), &FieldType::String("my_value".to_string()));

    // Modify it
    cache.insert_field_in_cache(
        "sale_order",
        "my_field",
        id_1.get_ids_ref(),
        Some(FieldType::String("my_value_2".to_string())),
        &Dirty::UpdateDirty,
        &Update::UpdateIfExists,
        &Compute::ResetCompute,
    );
    let cache_field = cache.get_field_from_cache("sale_order", "my_field", &id_1.get_id());
    assert!(cache_field.is_some());
    assert_eq!(cache_field.unwrap(), &FieldType::String("my_value_2".to_string()));

    // Clear the field
    cache.insert_field_in_cache(
        "sale_order",
        "my_field",
        id_1.get_ids_ref(),
        None,
        &Dirty::UpdateDirty,
        &Update::UpdateIfExists,
        &Compute::ResetCompute,
    );
    let cache_field = cache.get_field_from_cache("sale_order", "my_field", &id_1.get_id());
    assert!(cache_field.is_none());
    // Put field back
    cache.insert_field_in_cache(
        "sale_order",
        "my_field",
        id_1.get_ids_ref(),
        Some(FieldType::String("my_value_2".to_string())),
        &Dirty::UpdateDirty,
        &Update::UpdateIfExists,
        &Compute::ResetCompute,
    );

    // Insert another model
    cache
        .get_cache_models_mut("sale_order")
        .get_model_or_create(id_2.get_id());
    // Inserting another model shouldn't have modified the other field
    let cache_field = cache.get_field_from_cache("sale_order", "my_field", &id_1.get_id());
    assert!(cache_field.is_some());
    assert_eq!(cache_field.unwrap(), &FieldType::String("my_value_2".to_string()));

    // Modifying the other model shouldn't modify the other field
    cache.insert_field_in_cache(
        "sale_order",
        "my_field",
        id_2.get_ids_ref(),
        Some(FieldType::String("my_value_3".to_string())),
        &Dirty::UpdateDirty,
        &Update::UpdateIfExists,
        &Compute::ResetCompute,
    );
    let cache_field = cache.get_field_from_cache("sale_order", "my_field", &id_1.get_id());
    assert!(cache_field.is_some());
    assert_eq!(cache_field.unwrap(), &FieldType::String("my_value_2".to_string()));
    let cache_field = cache.get_field_from_cache("sale_order", "my_field", &id_2.get_id());
    assert!(cache_field.is_some());
    assert_eq!(cache_field.unwrap(), &FieldType::String("my_value_3".to_string()));
}

#[test]
fn test_x2x_fields() -> Result<()> {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.post_register();
    let mut env = app.new_env();

    // Create empty SO
    let map: MapOfFields = MapOfFields::default();
    let sale_order: SaleOrder<SingleId> = env.create_new_record_from_map(map)?;

    // Create empty SO line
    let map: MapOfFields = MapOfFields::default();
    let sale_order_line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;

    // SO shouldn't have any lines, as it's not linked
    // So, both methods should return an empty list
    assert!(sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?.id.is_empty());
    assert!(sale_order_line.get_order::<SaleOrder<_>>(&mut env)?.is_none());

    // Linking SO to a line should work, for both side
    // TODO Allow to add/remove line(s), instead of set
    // TODO Clean this, to only pass "sale_order_line" instead of "sale_order_line.id.clone().into()"
    sale_order.set_lines(sale_order_line.id.clone().into(), &mut env)?;
    assert!(sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?.id.contains(sale_order_line.id.get_id_ref()));
    assert_eq!(sale_order_line.get_order::<SaleOrder<_>>(&mut env)?.map(|order| order.id), Some(sale_order.id.clone()));

    // Let's add another line
    let map: MapOfFields = MapOfFields::default();
    let sale_order_line_2: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
    sale_order_line_2.set_order(Some(sale_order.id.get_id().into()), &mut env)?;
    assert!(sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?.id.contains(sale_order_line.id.get_id_ref()));
    assert!(sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?.id.contains(sale_order_line_2.id.get_id_ref()));
    assert_eq!(sale_order_line.get_order::<SaleOrder<_>>(&mut env)?.map(|order| order.id), Some(sale_order.id.clone()));
    assert_eq!(sale_order_line_2.get_order::<SaleOrder<_>>(&mut env)?.map(|order| order.id), Some(sale_order.id.clone()));

    // Also, creating a new line that has a direct link to a SO should also have the correct SO
    let mut map: MapOfFields = MapOfFields::default();
    // TODO Clean this, to allow to pass "sale_order" instead of "sale_order.id.get_id()"
    map.insert("order", sale_order.id.get_id());
    let sale_order_line_3: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
    assert!(sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?.id.contains(sale_order_line.id.get_id_ref()));
    assert!(sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?.id.contains(sale_order_line_2.id.get_id_ref()));
    // TODO Fix this (save in cache if needed)
    assert!(sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?.id.contains(sale_order_line_3.id.get_id_ref()));
    assert_eq!(sale_order_line.get_order::<SaleOrder<_>>(&mut env)?.map(|order| order.id), Some(sale_order.id.clone()));
    assert_eq!(sale_order_line_2.get_order::<SaleOrder<_>>(&mut env)?.map(|order| order.id), Some(sale_order.id.clone()));
    assert_eq!(sale_order_line_3.get_order::<SaleOrder<_>>(&mut env)?.map(|order| order.id), Some(sale_order.id.clone()));
    Ok(())
}
