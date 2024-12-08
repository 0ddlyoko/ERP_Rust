use erp::environment::Environment;
use erp::field::{EnumType, FieldDescriptor, FieldType};
use erp::model::{MapOfFields, Model, ModelDescriptor};
use std::error::Error;

#[derive(Default, Debug, PartialEq, Eq)]
pub enum PluginState {
    #[default]
    NotInstalled,
    Installed,
}

impl EnumType for PluginState {
    fn to_string(&self) -> String {
        match self {
            PluginState::Installed => String::from("installed"),
            PluginState::NotInstalled => String::from("not_installed"),
        }
    }

    fn from_string(t: String) -> Self {
        match t.as_ref() {
            "not_installed" => PluginState::NotInstalled,
            "installed" => PluginState::Installed,
            _ => PluginState::NotInstalled,
        }
    }
}

#[derive(Default)]
pub struct Plugin {
    id: u32,
    name: String,
    description: Option<String>,
    website: Option<String>,
    url: Option<String>,
    state: PluginState,
    // TODO Add module category
    // TODO Add author
    // TODO Add version (installed, latest, ...) + auto update if new version
}

impl Plugin {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn get_name(&self) -> &String {
        &self.name
    }

    fn get_description(&self) -> Option<&String> {
        self.description.as_ref()
    }

    fn get_website(&self) -> Option<&String> {
        self.website.as_ref()
    }

    fn get_url(&self) -> Option<&String> {
        self.url.as_ref()
    }

    fn get_state(&self) -> &PluginState {
        &self.state
    }
}

impl Model for Plugin {
    fn get_model_name() -> String {
        "country".to_string()
    }

    fn get_model_descriptor() -> ModelDescriptor {
        ModelDescriptor {
            name: "country".to_string(),
            description: Some("Countries".to_string()),
            fields: vec![
                FieldDescriptor {
                    name: "name".to_string(),
                    default_value: Some(FieldType::String("".to_string())),
                    description: Some("Name of the module".to_string()),
                    required: Some(true),
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "description".to_string(),
                    default_value: Some(FieldType::String("".to_string())),
                    description: Some("Description of the module".to_string()),
                    required: Some(false),
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "website".to_string(),
                    default_value: Some(FieldType::String("".to_string())),
                    description: Some("Website of the module".to_string()),
                    required: Some(false),
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "url".to_string(),
                    default_value: Some(FieldType::String("".to_string())),
                    description: Some("URL of the module".to_string()),
                    required: Some(false),
                    ..FieldDescriptor::default()
                },
                FieldDescriptor {
                    name: "state".to_string(),
                    default_value: Some(FieldType::Enum(PluginState::NotInstalled.to_string())),
                    description: Some("State of the module".to_string()),
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
        result.insert_option("description", self.get_description());
        result.insert_option("website", self.get_website());
        result.insert_option("url", self.get_url());
        result.insert("state", self.get_state());
        result
    }

    fn create_model(id: u32, data: MapOfFields) -> Self {
        Self {
            id,
            name: data.get("name"),
            description: data.get_option("description"),
            website: data.get_option("website"),
            url: data.get_option("url"),
            state: data.get("state"),
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
