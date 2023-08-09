use std::collections::HashMap;
use crate::field::FieldDescriptor;

#[derive(Debug)]
pub struct ModelDescriptor {
    pub table_name: String,
    pub fields: HashMap<String, FieldDescriptor>,
}

impl ModelDescriptor {
    pub fn name(&self) -> &String {
        &self.table_name
    }

    pub fn fields(&self) -> Vec<&FieldDescriptor> {
        self.fields.values().collect()
    }

    pub fn field(&self, field_name: &str) -> Option<&FieldDescriptor> {
        self.fields.get(field_name)
    }
}

pub trait InternalModelGetterDescriptor {
    fn _get_model_descriptor() -> ModelDescriptor;
}

pub struct ModelManager {
    models: HashMap<String, ModelDescriptor>,
}

impl ModelManager {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    pub fn register<IMD>(&mut self) where IMD: InternalModelGetterDescriptor {
        let model_descriptor = IMD::_get_model_descriptor();
        let table_name = model_descriptor.table_name.clone();
        if !self.models.contains_key(&table_name) {

        }
        self.models.insert(table_name, model_descriptor);
    }
}
