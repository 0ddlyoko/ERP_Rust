
#[derive(Debug)]
pub struct GeneratedFieldDescriptor {
    pub field_name: String,
    pub is_required: Option<bool>,
}

impl GeneratedFieldDescriptor {
    pub fn name(&self) -> &String {
        &self.field_name
    }

    pub fn required(&self) -> &Option<bool> {
        &self.is_required
    }
}

#[derive(Debug)]
pub struct FieldDescriptor {
    pub field_name: String,
    pub column_name: String,
    pub is_required: bool,
}

impl FieldDescriptor {
    pub fn default(field_name: &str) -> FieldDescriptor {
        FieldDescriptor {
            field_name: field_name.to_string(),
            column_name: field_name.to_string(),
            is_required: false,
        }
    }
}
