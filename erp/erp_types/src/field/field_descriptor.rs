use crate::field::{FieldCompute, FieldKind, FieldReference, FieldType};

#[derive(Default)]
pub struct FieldDescriptor {
    pub name: String,
    /// Type of the field. Carried explicitly rather than read off the default, so a field can
    /// have no default at all.
    pub kind: FieldKind,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: bool,
    pub compute: Option<FieldCompute>,
    pub field_ref: Option<FieldReference>,
}
