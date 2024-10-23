use core::environment::Environment;
use core::model::ModelManager;
use std::collections::HashMap;

struct SaleOrder {
    id: u32,
    name: String,
    total_price: i64,
}

impl SaleOrder {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn get_total_price(&self) -> i64 {
        self.total_price
    }
}

impl core::model::Model for SaleOrder {
    fn get_model_name() -> &'static str {
        "sale_order"
    }

    fn get_model_descriptor() -> core::model::ModelDescriptor {
        core::model::ModelDescriptor {
            name: "sale_order".to_string(),
            description: "A Sale Order!".to_string(),
            fields: vec![
                core::field::FieldDescriptor {
                    name: "name".to_string(),
                    field_type: core::field::FieldType::String(String::new()),
                    description: Some("Name of the SO".to_string()),
                    required: Some(true),
                    ..core::field::FieldDescriptor::default()
                },
                core::field::FieldDescriptor {
                    name: "total_price".to_string(),
                    field_type: core::field::FieldType::Integer(0),
                    description: Some("Total price of the SO".to_string()),
                    ..core::field::FieldDescriptor::default()
                },
            ],
        }
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_data(&self) -> core::model::MapOfFields {
        let mut map = HashMap::new();
        map.insert("name", Some(core::field::FieldType::String(self.name.clone())));
        map.insert("total_price", Some(core::field::FieldType::Integer(self.total_price)));
        map
    }

    fn create_model(id: u32, data: core::model::MapOfFields) -> Self {
        let name = data["name"].clone().unwrap().string();
        let total_price = data["total_price"].clone().unwrap().integer();
        Self {
            id,
            name,
            total_price,
        }
    }
}

#[test]
fn test_get_record() {
    let mut model_manager = ModelManager::new();
    model_manager.register_model::<SaleOrder>();
    let mut env = Environment::new(&model_manager);

    // Insert random data inside
    let mut map = HashMap::new();
    map.insert("name", Some(core::cache::CacheFieldValue::String("0ddlyoko".to_string())));
    map.insert("total_price", Some(core::cache::CacheFieldValue::Int(42)));
    env.cache.insert_record_model_with_map("sale_order", 1, map);

    // Get the record
    let sale_order = env.get_record::<SaleOrder>(1);
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
