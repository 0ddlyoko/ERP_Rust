use std::collections::HashMap;

pub struct SaleOrder {
    pub id: u32,
    pub name: String,
    pub total_price: i64,
}

impl SaleOrder {
    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_total_price(&self) -> i64 {
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
            description: Some("A Sale Order!".to_string()),
            fields: vec![
                core::field::FieldDescriptor {
                    name: "name",
                    default_value: Some(core::field::FieldType::String("0ddlyoko".to_string())),
                    description: Some("Name of the SO".to_string()),
                    required: Some(true),
                    ..core::field::FieldDescriptor::default()
                },
                core::field::FieldDescriptor {
                    name: "total_price",
                    default_value: Some(core::field::FieldType::Integer(42)),
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
