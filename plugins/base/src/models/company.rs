use erp::environment::Environment;
use erp::field::{FieldDescriptor, FieldType};
use erp::model::{BaseModel, MapOfFields, Model, ModelDescriptor, SimplifiedModel};
use std::error::Error;

pub struct BaseCompany;

impl BaseModel for BaseCompany {
    fn get_model_name() -> &'static str {
        "company"
    }
}

pub struct Company {
    id: u32,
    name: String,
    // TODO Link to contact
}

impl Company {
    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }
}

impl Model for Company {
    type BaseModel = BaseCompany;
}

impl SimplifiedModel for Company {
    fn get_model_descriptor() -> ModelDescriptor {
        ModelDescriptor {
            name: Self::get_model_name().to_string(),
            description: Some("Companies".to_string()),
            fields: vec![FieldDescriptor {
                name: "name".to_string(),
                default_value: Some(FieldType::String("".to_string())),
                description: Some("Name of the company".to_string()),
                required: true,
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
        env.save_record_from_name("test", self);
        Ok(())
    }
}
