use std::error::Error;
use erp::environment::Environment;
use erp::field::FieldDescriptor;
use erp::field::FieldType;
use erp::model::{MapOfFields, Model, ModelDescriptor};

#[derive(Default)]
pub(crate) struct SaleOrderTest {
    id: u32,
    name: String,
    age: i64,
}

impl SaleOrderTest {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_name(&self) -> &String {
        &self.name
    }

    fn get_age(&self) -> i64 {
        self.age
    }

    fn _compute_age(&mut self, env: &mut Environment) -> Result<(), Box<dyn Error>> {
        self.age = 42;
        Ok(())
    }
}

impl Model for SaleOrderTest {

    fn get_model_name() -> String {
        "sale_order_test".to_string()
    }

    fn get_model_descriptor() -> ModelDescriptor {
        ModelDescriptor {
            name: "sale_order_test".to_string(),
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
                    name: "age".to_string(),
                    default_value: Some(FieldType::Integer(0)),
                    description: Some("Age of the logged user".to_string()),
                    required: Some(true),
                    compute: Some(true),
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
        result.insert("name", self.get_name());
        result.insert("age", self.get_age());
        result
    }

    fn create_model(id: u32, data: MapOfFields) -> Self {
        Self {
            id,
            name: data.get("name"),
            age: data.get("age"),
        }
    }

    fn call_compute_method(&mut self, field_name: &str, env: &mut Environment) -> Result<(), Box<dyn Error>> {
        if field_name == "age" {
            return self._compute_age(env);
        }
        Ok(())
    }
}
