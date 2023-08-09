
#[derive(Debug)]
pub struct FieldDescriptor {
    pub field_name: String,
    pub is_required: bool,
}

impl FieldDescriptor {
    pub fn name(&self) -> &String {
        &self.field_name
    }

    pub fn required(&self) -> bool {
        self.is_required
    }
}
