use std::{error, fmt};

#[derive(Debug, Clone)]
pub struct ModelNotFound {
    pub model_name: String,
}

impl fmt::Display for ModelNotFound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Model \"{}\" is not present in registries!", self.model_name)
    }
}

impl error::Error for ModelNotFound {}
