use crate::cache::errors::RecordNotFoundError;
use crate::environment::Environment;
use crate::model::Model;
use std::error::Error;

pub struct Reference {
    pub id: u32,
}

impl Reference {

    /// Retrieves the instance of this ref
    /// mut is needed as we could load the given ref into the cache.
    /// If there is any errors while loading the record, the error is returned.
    /// If the record is of type "RecordNotFoundError", Ok(None) is returned instead
    pub fn get<E: Model>(&mut self, env: &mut Environment) -> Result<Option<E>, Box<dyn Error>> {
        let result = env.get_record::<E>(self.id);
        if let Err(error) = result {
            if error.downcast_ref::<RecordNotFoundError>().is_some() {
                return Ok(None);
            }
            return Err(error);
        }
        let result = result?;
        Ok(Some(result))
    }
}

impl From<u32> for Reference {
    fn from(value: u32) -> Self {
        Reference {
            id: value,
        }
    }
}

impl From<&u32> for Reference {
    fn from(value: &u32) -> Self {
        Reference {
            id: *value,
        }
    }
}
