use crate::field::{FieldType, FieldReference};

#[derive(Default)]
pub struct FieldDescriptor {
    pub name: String,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: bool,
    pub compute: Option<bool>,
    pub depends: Option<Vec<String>>,
    pub field_ref: Option<FieldReference>,
}
