use std::{error, fmt};

#[derive(Debug, Clone)]
pub struct MaximumRecursionDepthCompute {
    pub model_name: String,
    pub fields_name: Vec<String>,
    pub ids: Vec<u32>,
}

impl fmt::Display for MaximumRecursionDepthCompute {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Maximum recursion depth while computing fields {}.{:?}. Combined ids are: {:?}",
            self.model_name, self.fields_name, self.ids
        )
    }
}

impl error::Error for MaximumRecursionDepthCompute {}
