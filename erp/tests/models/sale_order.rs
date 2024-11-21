use erp::field::{FieldDescriptor, FieldType};
use erp::model::{MapOfFields, Model, ModelDescriptor};

#[derive(Default)]
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

impl Model for SaleOrder {
    fn get_model_name() -> String {
        "sale_order".to_string()
    }

    fn get_model_descriptor() -> ModelDescriptor<SaleOrder> {
        ModelDescriptor {
            name: "sale_order".to_string(),
            description: Some("A Sale Order!".to_string()),
            fields: vec![
                FieldDescriptor {
                    name: "name".to_string(),
                    default_value: Some(FieldType::String("0ddlyoko".to_string())),
                    description: Some("Name of the SO".to_string()),
                    required: Some(true),
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "total_price".to_string(),
                    default_value: Some(FieldType::Integer(42)),
                    description: Some("Total price of the SO".to_string()),
                    ..FieldDescriptor::default()
                },
            ],
        }
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_data(&self) -> MapOfFields {
        let mut result = MapOfFields::default();
        result.insert("name", &self.name);
        result.insert("total_price", self.total_price);
        result
    }

    fn create_model(id: u32, data: MapOfFields) -> Self {
        Self {
            id,
            name: data.get("name"),
            total_price: data.get("total_price"),
        }
    }
}
