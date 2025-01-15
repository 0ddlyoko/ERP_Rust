use std::{error, fmt};

#[derive(Debug, Clone)]
pub struct RequiredFieldEmpty {
    pub model_name: String,
    pub field_name: String,
    pub id: u32,
}

impl fmt::Display for RequiredFieldEmpty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Field \"{}\".\"{}\" for record \"{}\" is required but is empty. This should not happen",
            self.model_name,
            self.field_name,
            self.id,
        )
    }
}

impl error::Error for RequiredFieldEmpty {}
