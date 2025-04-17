use std::{error, fmt};

#[derive(Debug, Clone)]
pub struct RecordsNotFoundError {
    pub(crate) model_name: String,
    pub(crate) ids: Vec<u32>,
}

impl fmt::Display for RecordsNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Records {:?} not found for model {}",
            self.ids, self.model_name
        )
    }
}

impl error::Error for RecordsNotFoundError {}
