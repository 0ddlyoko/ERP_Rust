use std::collections::HashMap;

pub(crate) struct SaleOrderTest {
    id: u32,
    name: String,
}

impl SaleOrderTest {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }
}

impl core::model::Model for SaleOrderTest {
    fn get_model_name() -> &'static str {
        "sale_order_test"
    }

    fn get_model_descriptor() -> core::model::ModelDescriptor {
        core::model::ModelDescriptor {
            name: "sale_order_test".to_string(),
            description: Some("A Sale Order!".to_string()),
            fields: vec![
                core::field::FieldDescriptor {
                    name: "name",
                    default_value: Some(core::field::FieldType::String("0ddlyoko".to_string())),
                    description: Some("Name of the SO".to_string()),
                    required: Some(true),
                    ..core::field::FieldDescriptor::default()
                }
            ],
        }
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_data(&self) -> core::model::MapOfFields {
        let mut map = HashMap::new();
        map.insert("name", Some(core::field::FieldType::String(self.name.clone())));
        map
    }

    fn create_model(id: u32, data: core::model::MapOfFields) -> Self {
        let name = data["name"].clone().unwrap().string();
        Self {
            id,
            name,
        }
    }
}