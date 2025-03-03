use crate::field::{FieldType, FieldReference, FieldCompute};

#[derive(Default)]
pub struct FieldDescriptor {
    pub name: String,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: bool,
    pub compute: Option<FieldCompute>,
    pub field_ref: Option<FieldReference>,
}
