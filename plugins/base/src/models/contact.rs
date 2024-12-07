use erp::environment::Environment;
use erp::field::{FieldDescriptor, FieldType};
use erp::model::{MapOfFields, Model, ModelDescriptor};
use std::error::Error;

#[derive(Default)]
pub struct Contact {
    id: u32,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
    // TODO link to lang
}

impl Contact {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_name(&self) -> &String {
        &self.name
    }

    fn get_email(&self) -> Option<&String> {
        self.email.as_ref()
    }

    fn get_phone(&self) -> Option<&String> {
        self.phone.as_ref()
    }

    fn get_website(&self) -> Option<&String> {
        self.website.as_ref()
    }
}

impl Model for Contact {
    fn get_model_name() -> String {
        "contact".to_string()
    }

    fn get_model_descriptor() -> ModelDescriptor {
        ModelDescriptor {
            name: "contact".to_string(),
            description: Some("Contact".to_string()),
            fields: vec![
                FieldDescriptor {
                    name: "name".to_string(),
                    default_value: Some(FieldType::String("".to_string())),
                    description: Some("Name of the contact".to_string()),
                    required: Some(false),
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "email".to_string(),
                    default_value: Some(FieldType::String("".to_string())),
                    description: Some("Email of the contact".to_string()),
                    required: Some(false),
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "phone".to_string(),
                    default_value: Some(FieldType::String("".to_string())),
                    description: Some("Phone number of the contact".to_string()),
                    required: Some(false),
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "website".to_string(),
                    default_value: Some(FieldType::String("".to_string())),
                    description: Some("Website of the contact".to_string()),
                    required: Some(false),
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
        result.insert_option("email", self.get_email());
        result.insert_option("phone", self.get_phone());
        result.insert_option("website", self.get_website());
        result
    }

    fn create_model(id: u32, data: MapOfFields) -> Self {
        Self {
            id,
            name: data.get("name"),
            email: data.get_option("email"),
            phone: data.get_option("phone"),
            website: data.get_option("website"),
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
