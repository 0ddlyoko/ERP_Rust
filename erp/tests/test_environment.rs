use erp::app::Application;
use erp::cache::{Dirty, Update};
use erp::database::Database;
use erp::field::{FieldType, IdMode, SingleId};
use erp::model::MapOfFields;
use erp_search_code_gen::make_domain;
use std::collections::HashMap;
use std::error::Error;
use test_utilities::models::{SaleOrder, SaleOrderLine, SaleOrderState};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[test]
fn test_fill_default_values_on_map() -> Result<()> {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.post_register();
    let env = app.new_env();

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
    Ok(())
}

#[test]
fn test_get_fields_to_save() -> Result<()> {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.post_register();
    let env = app.new_env();

    // Empty
    assert_eq!(env.get_fields_to_save("sale_order", &vec![

    ])?.into_iter().map(|(k, mut v)| { v.sort(); (k, v) }).collect::<HashMap<_, _>>(), HashMap::from([
    ]));

    assert_eq!(env.get_fields_to_save("sale_order", &vec![
        &("name".into()),
    ])?.into_iter().map(|(k, mut v)| { v.sort(); (k, v) }).collect::<HashMap<_, _>>(), HashMap::from([
        ("sale_order", vec!["name"]),
    ]));

    assert_eq!(env.get_fields_to_save("sale_order", &vec![
        &("name".into()),
        &("state".into()),
        &("total_price".into()),
    ])?.into_iter().map(|(k, mut v)| { v.sort(); (k, v) }).collect::<HashMap<_, _>>(), HashMap::from([
        ("sale_order", vec!["name", "state", "total_price"]),
    ]));

    // Give same fields should only appear once
    assert_eq!(env.get_fields_to_save("sale_order", &vec![
        &("name".into()),
        &("name".into()),
        &("name".into()),
    ])?.into_iter().map(|(k, mut v)| { v.sort(); (k, v) }).collect::<HashMap<_, _>>(), HashMap::from([
        ("sale_order", vec!["name"]),
    ]));

    // Path should also work correctly, and only add stored fields
    // Here, "order" is returned instead of "lines" as "order" is stored, and not "lines"
    assert_eq!(env.get_fields_to_save("sale_order", &vec![
        &("lines.price".into()),
        &("lines.amount".into()),
        &("lines.total_price".into()),
    ])?.into_iter().map(|(k, mut v)| { v.sort(); (k, v) }).collect::<HashMap<_, _>>(), HashMap::from([
        ("sale_order_line", vec!["amount", "order", "price", "total_price"]),
    ]));

    // Same here, as "order" is a stored field, this should be returned (and not "lines")
    assert_eq!(env.get_fields_to_save("sale_order_line", &vec![
        &("price".into()),
        &("order.name".into()),
    ])?.into_iter().map(|(k, mut v)| { v.sort(); (k, v) }).collect::<HashMap<_, _>>(), HashMap::from([
        ("sale_order", vec!["name"]),
        ("sale_order_line", vec!["order", "price"]),
    ]));

    Ok(())
}

#[test]
fn test_get_record() -> Result<()> {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.post_register();
    let mut env = app.new_env();

    // Create new record with default values
    let map: MapOfFields = MapOfFields::default();
    let sale_order_line: SaleOrderLine<_> = env.create_new_record_from_map(map)?;
    let id = sale_order_line.get_id();

    assert_eq!(*sale_order_line.get_price(&mut env)?, 42);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 10);
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 42 * 10, "Should not be 0 as the computed method is called");
    let price_cache_record = env.cache.get_field_from_cache("sale_order_line", "price", &id);
    let amount_cache_record = env.cache.get_field_from_cache("sale_order_line", "amount", &id);
    assert!(price_cache_record.is_some());
    assert!(amount_cache_record.is_some());
    let price_cache_record = price_cache_record.unwrap();
    let amount_cache_record = amount_cache_record.unwrap();
    assert_eq!(*price_cache_record, FieldType::Integer(42));
    assert_eq!(*amount_cache_record, FieldType::Integer(10));
    // Dirty
    let dirty_fields = env.cache.get_cache_models("sale_order_line").get_dirty(&id);
    assert!(dirty_fields.is_some());
    // We should have "total_price" as a dirty field, as this field hasn't been saved in the database
    assert_eq!(dirty_fields.unwrap().len(), 1);
    assert!(dirty_fields.unwrap().contains("total_price"));
    // For the following tests, we will push "total_price" to the database
    env.save_records_to_db("sale_order_line", &sale_order_line.id)?;

    // Changing the price should alter the cache
    sale_order_line.set_price(50, &mut env)?;

    let price_cache_record = env.cache.get_field_from_cache("sale_order_line", "price", &id);
    let amount_cache_record = env.cache.get_field_from_cache("sale_order_line", "amount", &id);
    assert!(price_cache_record.is_some());
    assert!(amount_cache_record.is_some());
    let price_cache_record = price_cache_record.unwrap();
    let amount_cache_record = amount_cache_record.unwrap();
    assert_eq!(*price_cache_record, FieldType::Integer(50));
    assert_eq!(*amount_cache_record, FieldType::Integer(10));
    // Price has been modified, it should be dirty
    let dirty_fields = env.cache.get_cache_models("sale_order_line").get_dirty(&id);
    assert!(dirty_fields.is_some());
    let dirty_fields = dirty_fields.unwrap();
    assert_eq!(dirty_fields.len(), 1);
    assert!(dirty_fields.contains(&"price".to_string()));
    let cache_models = env.cache.get_cache_models_mut("sale_order_line");
    assert!(cache_models.get_model(&id).is_some());
    let dirty_fields = cache_models.get_dirty(&id);
    assert!(dirty_fields.is_some());
    assert!(dirty_fields
        .unwrap()
        .iter()
        .eq(["price".to_string()].iter()));

    // Clear dirty
    cache_models.clear_dirty(&[id]);
    assert!(cache_models.get_dirty(&id).is_none());

    Ok(())
}

#[test]
fn test_get_record_from_xxx() -> Result<()> {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.post_register();
    let mut env = app.new_env();

    // Insert random data inside
    let mut map: MapOfFields = MapOfFields::default();
    env.fill_default_values_on_map("sale_order", &mut map);

    env.cache.insert_fields_in_cache("sale_order", 1, map, &Dirty::NotUpdateDirty, &Update::UpdateIfExists);

    // Get the record
    let sale_order = env.get_record::<SaleOrder<_>, _>(1.into());

    assert_eq!(sale_order.get_id(), 1);
    assert_eq!(sale_order.get_name(&mut env)?, "0ddlyoko");
    Ok(())
}

#[test]
fn test_compute_method() -> Result<()> {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.post_register();
    let mut env = app.new_env();

    // Insert random data inside
    let map: MapOfFields = MapOfFields::default();
    let sale_order_line: SaleOrderLine<_> = env.create_new_record_from_map(map)?;
    let id = sale_order_line.get_id();

    // Get the record
    assert_eq!(*sale_order_line.get_price(&mut env)?, 42);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 10);
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 42 * 10);

    // Modifying a key should call the computed method when we need it
    sale_order_line.set_price(50, &mut env)?;
    let cache_value = env.cache.get_field_from_cache("sale_order_line", "total_price", &id);
    assert!(cache_value.is_some());
    assert_eq!(cache_value.unwrap(), &FieldType::Integer(42 * 10), "Total price shouldn't be updated if we don't want the new value");

    assert_eq!(sale_order_line.id, id);
    assert_eq!(*sale_order_line.get_price(&mut env)?, 50);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 10);
    // Here, it should be updated
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 50 * 10);
    Ok(())
}

#[test]
fn save_fields_to_db() -> Result<()> {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.post_register();
    let mut env = app.new_env();

    let map: MapOfFields = MapOfFields::default();
    let sale_order: SaleOrder<SingleId> = env.create_new_record_from_map(map)?;
    let id = sale_order.get_id();

    // create_new_record_from_map should create the record in database
    let sale_order_vec = env.database.search("sale_order", &["name", "state", "total_price"], &make_domain!([("name", "=", "0ddlyoko")]), env.model_manager)?;
    assert!(!sale_order_vec.is_empty());
    // Default values should be applied here
    assert_eq!(sale_order_vec.len(), 1);
    let (a, b) = sale_order_vec.first().unwrap();
    assert_eq!(*a, id);
    assert_eq!(b.get("name"), Some(&Some(erp::database::FieldType::String("0ddlyoko".to_string()))));
    assert_eq!(b.get("state"), Some(&Some(erp::database::FieldType::String("draft".to_string()))));
    assert_eq!(b.get("total_price"), Some(&Some(erp::database::FieldType::Integer(0))));


    // create_new_record_from_map should create the record, and so this should not be dirty
    assert!(!env.cache.get_cache_models("sale_order").is_field_dirty("name", &id));

    // Changing the name should set this field as dirty
    sale_order.set_name("1ddlyoko".to_string(), &mut env)?;
    sale_order.set_state(SaleOrderState::Paid, &mut env)?;
    sale_order.set_total_price(42, &mut env)?;
    assert!(env.cache.get_cache_models("sale_order").is_field_dirty("name", &id));
    assert!(env.cache.get_cache_models("sale_order").is_field_dirty("state", &id));
    assert!(env.cache.get_cache_models("sale_order").is_field_dirty("total_price", &id));
    // To Recompute shouldn't be set for those fields
    assert!(!env.cache.is_field_to_recompute("sale_order", "name", &id));
    assert!(!env.cache.is_field_to_recompute("sale_order", "state", &id));
    // Neither for total_price, as we fixed a value before
    assert!(!env.cache.is_field_to_recompute("sale_order", "total_price", &id));

    // Calling save_records_to_db should save given records to db, and so only those records should not be dirty anymore
    env.save_fields_to_db("sale_order", &["name", "state"])?;
    let cache_models = env.cache.get_cache_models("sale_order");
    assert!(!cache_models.is_field_dirty("name", &id));
    assert!(!cache_models.is_field_dirty("state", &id));
    assert!(cache_models.is_field_dirty("total_price", &id));

    // Those fields should be kept in cache
    let cache_model = cache_models.get_model(&id);
    assert!(cache_model.is_some());
    let cache_model = cache_model.unwrap();
    assert!(cache_model.get_field("name").is_some());
    assert_eq!(cache_model.get_field("name").unwrap().get(), Some(&FieldType::String("1ddlyoko".to_string())));
    assert!(cache_model.get_field("state").is_some());
    assert_eq!(cache_model.get_field("state").unwrap().get(), Some(&FieldType::String("paid".to_string())));

    // They should also be kept in database
    let sale_order_vec = env.database.search("sale_order", &["name", "state"], &make_domain!([("name", "=", "1ddlyoko")]), env.model_manager)?;
    assert!(!sale_order_vec.is_empty());
    assert_eq!(sale_order_vec.len(), 1);
    let (a, b) = sale_order_vec.first().unwrap();
    assert_eq!(*a, id);
    assert_eq!(b.get("name"), Some(&Some(erp::database::FieldType::String("1ddlyoko".to_string()))));
    assert_eq!(b.get("state"), Some(&Some(erp::database::FieldType::String("paid".to_string()))));
    // We don't retrieve this field, so it should not be in the map
    assert_eq!(b.get("total_price"), None);

    Ok(())
}

#[test]
fn search() -> Result<()> {
    let mut app = Application::new_test();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.post_register();
    let mut env = app.new_env();

    // SO
    let map: MapOfFields = MapOfFields::default();
    let sale_order: SaleOrder<SingleId> = env.create_new_record_from_map(map)?;

    // SO Line
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("order", sale_order.id.get_id());
    let sale_order_line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;

    // create_new_record_from_map should create the record in database
    let sale_order_search = env.search::<SaleOrder<_>>(&make_domain!([("name", "=", "0ddlyoko")]))?;
    assert_eq!(sale_order_search.id, sale_order.id);

    // We can also search on sale_order_line
    let sale_order_line_search = env.search::<SaleOrderLine<_>>(&make_domain!([("order.name", "=", "0ddlyoko")]))?;
    assert_eq!(sale_order_line_search.id, sale_order_line.id);

    assert_eq!(*sale_order.get_name(&mut env)?, "0ddlyoko".to_string());
    assert_eq!(*sale_order.get_state(&mut env)?, SaleOrderState::Draft);
    assert_eq!(*sale_order.get_total_price(&mut env)?, 420);

    // Update a record should still return this record
    sale_order.set_name("1ddlyoko".to_string(), &mut env)?;

    // Old search should return nothing
    assert!(env.search::<SaleOrder<_>>(&make_domain!([("name", "=", "0ddlyoko")]))?.id.is_empty());
    assert!(env.search::<SaleOrderLine<_>>(&make_domain!([("order.name", "=", "0ddlyoko")]))?.id.is_empty());
    // Because name is now "1ddlyoko"
    assert_eq!(sale_order_search.id, env.search::<SaleOrder<_>>(&make_domain!([("name", "=", "1ddlyoko")]))?.id);
    assert_eq!(sale_order_line_search.id, env.search::<SaleOrderLine<_>>(&make_domain!([("order.name", "=", "1ddlyoko")]))?.id);


    // Update the line, and check if we can search on it
    sale_order_line.set_amount(42, &mut env)?;
    assert!(env.search::<SaleOrder<_>>(&make_domain!([("lines.amount", "=", 50)]))?.id.is_empty());
    assert!(env.search::<SaleOrder<_>>(&make_domain!([("lines.amount", "=", 69)]))?.id.is_empty());
    assert_eq!(sale_order_search.id, env.search::<SaleOrder<_>>(&make_domain!([("lines.amount", "=", 42)]))?.id);

    // If we add a new line with amount = 69, this should work
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("order", sale_order.id.get_id());
    map.insert("amount", 69);
    let _sale_order_line_2: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
    assert!(env.search::<SaleOrder<_>>(&make_domain!([("lines.amount", "=", 50)]))?.id.is_empty());
    assert_eq!(sale_order_search.id, env.search::<SaleOrder<_>>(&make_domain!([("lines.amount", "=", 69)]))?.id);
    assert_eq!(sale_order_search.id, env.search::<SaleOrder<_>>(&make_domain!([("lines.amount", "=", 42)]))?.id);


    // We should also be able to make a complex query
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("name", "name_1");
    let so_1 = env.create_new_record_from_map::<SaleOrder<_>>(map)?;
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("order", so_1.id.get_id());
    map.insert("price", 10);
    let _line_1_1 = env.create_new_record_from_map::<SaleOrderLine<_>>(map)?;
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("order", so_1.id.get_id());
    map.insert("price", 49);
    let _line_1_2 = env.create_new_record_from_map::<SaleOrderLine<_>>(map)?;
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("order", so_1.id.get_id());
    map.insert("price", 10000);
    let _line_1_3 = env.create_new_record_from_map::<SaleOrderLine<_>>(map)?;

    let mut map: MapOfFields = MapOfFields::default();
    map.insert("name", "name_2");
    let so_2 = env.create_new_record_from_map::<SaleOrder<_>>(map)?;
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("order", so_2.id.get_id());
    map.insert("price", 40);
    let _line_2_1 = env.create_new_record_from_map::<SaleOrderLine<_>>(map)?;
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("order", so_2.id.get_id());
    map.insert("price", 25);
    let _line_2_2 = env.create_new_record_from_map::<SaleOrderLine<_>>(map)?;
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("order", so_2.id.get_id());
    map.insert("price", 98);
    let _line_2_3 = env.create_new_record_from_map::<SaleOrderLine<_>>(map)?;

    let mut map: MapOfFields = MapOfFields::default();
    map.insert("name", "name_3");
    let so_3 = env.create_new_record_from_map::<SaleOrder<_>>(map)?;
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("order", so_3.id.get_id());
    map.insert("price", 75);
    let line_3_1 = env.create_new_record_from_map::<SaleOrderLine<_>>(map)?;
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("order", so_3.id.get_id());
    map.insert("price", 36);
    let line_3_2 = env.create_new_record_from_map::<SaleOrderLine<_>>(map)?;

    let mut map: MapOfFields = MapOfFields::default();
    map.insert("name", "name_4");
    let so_4 = env.create_new_record_from_map::<SaleOrder<_>>(map)?;
    let mut map: MapOfFields = MapOfFields::default();
    map.insert("order", so_4.id.get_id());
    map.insert("price", 36);
    let line_4_1 = env.create_new_record_from_map::<SaleOrderLine<_>>(map)?;

    let ids = vec![so_3.id.clone(), so_4.id.clone()];
    assert_eq!(ids, {
        let mut ids = env.search::<SaleOrder<_>>(&make_domain!([("lines.price", "=", 36)]))?.id.get_ids_ref().clone();
        ids.sort();
        ids
    });

    let ids = vec![sale_order.id.clone(), so_1.id.clone(), so_2.id.clone(), so_3.id.clone(), so_4.id.clone()];
    assert_eq!(ids, {
        let mut ids = env.search::<SaleOrder<_>>(&make_domain!(["&", ("lines.price", ">=", 30), ("lines.price", "<=", 50)]))?.id.get_ids_ref().clone();
        ids.sort();
        ids
    });

    let ids = vec![line_3_1.id.clone(), line_3_2.id.clone(), line_4_1.id.clone()];
    assert_eq!(ids, {
        let mut ids = env.search::<SaleOrderLine<_>>(&make_domain!([("order.lines.price", "=", 36)]))?.id.get_ids_ref().clone();
        ids.sort();
        ids
    });

    Ok(())
}
