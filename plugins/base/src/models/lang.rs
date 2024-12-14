use erp::environment::Environment;
use erp::field::{FieldDescriptor, FieldType};
use erp::model::{BaseModel, MapOfFields, Model, ModelDescriptor, SimplifiedModel};
use std::error::Error;

pub struct BaseLang;

impl BaseModel for BaseLang {
    fn get_model_name() -> &'static str {
        "lang"
    }
}

pub struct Lang {
    id: u32,
    name: String,
    code: String,
}

impl Lang {
    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_code(&self) -> &String {
        &self.code
    }
}

impl Model for Lang {
    type BaseModel = BaseLang;}

impl SimplifiedModel for Lang {
    fn get_model_descriptor() -> ModelDescriptor {
        ModelDescriptor {
            name: Self::get_model_name().to_string(),
            description: Some("Language".to_string()),
            fields: vec![
                FieldDescriptor {
                    name: "name".to_string(),
                    default_value: Some(FieldType::String("".to_string())),
                    description: Some("Name of the language".to_string()),
                    required: Some(true),
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "code".to_string(),
                    default_value: Some(FieldType::String("".to_string())),
                    description: Some("Code of the language".to_string()),
                    required: Some(true),
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
        result.insert("code", self.get_code());
        result
    }

    fn create_model(id: u32, data: MapOfFields) -> Self {
        Self {
            id,
            name: data.get("name"),
            code: data.get("code"),
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
