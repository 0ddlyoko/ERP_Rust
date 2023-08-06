
#[derive(Debug)]
pub struct Field {
    field_name: String,
    is_required: bool,
}

impl Field {
    pub fn name(&self) -> &String {
        &self.field_name
    }

    pub fn required(&self) -> bool {
        self.is_required
    }
}
