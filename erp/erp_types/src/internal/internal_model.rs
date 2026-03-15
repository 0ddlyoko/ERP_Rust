use std::collections::HashMap;
use std::error::Error;
use crate::environment::ErasedEnvironment;
use crate::field::MultipleIds;
use crate::internal::InternalField;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Model descriptor represented by a single struct model
pub struct InternalModel {
    pub name: String,
    pub description: Option<String>,
    pub fields: HashMap<String, InternalField>,
    pub computed_method: fn(&str, MultipleIds, &mut dyn ErasedEnvironment) -> Result<()>,
    pub plugin_name: String,
}

impl InternalModel {
    pub fn call_computed_method(
        &self,
        field_name: &str,
        id: MultipleIds,
        env: &mut dyn ErasedEnvironment,
    ) -> Result<()> {
        (self.computed_method)(field_name, id, env)
    }
}
