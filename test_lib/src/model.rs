use std::collections::HashMap;
use crate::field::Field;

#[derive(Debug)]
pub struct Model {
    table_name: String,
    fields: HashMap<String, Field>,
}

impl Model {
    pub fn name(&self) -> &String {
        &self.table_name
    }

    pub fn fields(&self) -> Vec<&Field> {
        self.fields.values().collect()
    }

    pub fn field(&self, field_name: &str) -> Option<&Field> {
        self.fields.get(field_name)
    }
}

pub trait InternalModel {
    fn _get_model() ->&'static str;
}
