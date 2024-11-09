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
    fn get_model_name() -> String {
        "sale_order_test".to_string()
    }

    fn get_model_descriptor() -> core::model::ModelDescriptor {
        core::model::ModelDescriptor {
            name: "sale_order_test".to_string(),
            description: Some("A Sale Order!".to_string()),
            fields: vec![
                core::field::FieldDescriptor {
                    name: "name".to_string(),
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
        let mut result = core::model::MapOfFields::default();
        result.insert("name", &self.name);
        result
    }

    fn create_model(id: u32, data: core::model::MapOfFields) -> Self {
        Self {
            id,
            name: data.get("name"),
        }
    }
}