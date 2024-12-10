use erp::environment::Environment;
use erp::field::{FieldDescriptor, FieldType};
use erp::model::{MapOfFields, Model, ModelDescriptor};
use std::error::Error;

pub struct Company {
    id: u32,
    name: String,
    // TODO Link to contact
}

impl Company {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_name(&self) -> &String {
        &self.name
    }
}

impl Model for Company {
    fn get_model_name() -> String {
        "country".to_string()
    }

    fn get_model_descriptor() -> ModelDescriptor {
        ModelDescriptor {
            name: "country".to_string(),
            description: Some("Countries".to_string()),
            fields: vec![FieldDescriptor {
                name: "name".to_string(),
                default_value: Some(FieldType::String("".to_string())),
                description: Some("Name of the country".to_string()),
                required: Some(true),
                ..FieldDescriptor::default()
            }],
        }
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_data(&self) -> MapOfFields {
        let mut result = MapOfFields::default();
        result.insert("name", self.get_name());
        result
    }

    fn create_model(id: u32, data: MapOfFields) -> Self {
        Self {
            id,
            name: data.get("name"),
        }
    }

    fn call_compute_method(
        &mut self,
        field_name: &str,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}
