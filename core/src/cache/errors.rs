use std::{error, fmt};

#[derive(Debug, Clone)]
pub struct RecordNotFoundError {
    pub(crate) model_name: &'static str,
    pub(crate) id: u32,
}

impl fmt::Display for RecordNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Cache {} not found for model {}", self.id, self.model_name)
    }
}

impl error::Error for RecordNotFoundError {}
