use crate::field::FieldType;

#[derive(Default)]
pub struct FieldDescriptor {
    pub name: String,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: Option<bool>,
    pub compute: Option<bool>,
}