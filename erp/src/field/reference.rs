use crate::cache::errors::RecordNotFoundError;
use crate::environment::Environment;
use crate::model::Model;
use std::error::Error;

pub struct Reference<E: Model> {
    pub result: Option<E>,
    pub id: u32,
    pub loaded: bool,
}

impl<E: Model> Reference<E> {

    /// Retrieves the instance of this ref
    /// mut is needed as we could load the given ref into the cache.
    /// If there is any errors while loading the record, the error is returned.
    /// If the record is of type "RecordNotFoundError", Ok(None) is returned instead
    fn get(&mut self, env: &mut Environment) -> Result<&Option<E>, Box<dyn Error>> {
        if self.loaded {
            return Ok(&self.result);
        }
        let result = env.get_record::<E>(self.id);
        if let Err(error) = result {
            if error.downcast_ref::<RecordNotFoundError>().is_some() {
                self.result = None;
                self.loaded = true;
                return Ok(&self.result);
            }
            return Err(error);
        }
        let result = result?;
        self.result = Some(result);
        self.loaded = true;
        Ok(&self.result)
    }

    /// Retrieves the instance of this ref
    /// mut is needed as we could load the given ref into the cache.
    /// If there is any errors while loading the record, the error is returned.
    /// If the record is of type "RecordNotFoundError", Ok(None) is returned instead
    fn get_mut(&mut self, env: &mut Environment) -> Result<&mut Option<E>, Box<dyn Error>> {
        if self.loaded {
            return Ok(&mut self.result);
        }
        let result = env.get_record::<E>(self.id);
        if let Err(error) = result {
            if error.downcast_ref::<RecordNotFoundError>().is_some() {
                self.result = None;
                self.loaded = true;
                return Ok(&mut self.result);
            }
            return Err(error);
        }
        let result = result?;
        self.result = Some(result);
        self.loaded = true;
        Ok(&mut self.result)
    }
}

impl<E: Model> From<u32> for Reference<E> {
    fn from(value: u32) -> Self {
        Reference {
            result: None,
            id: value,
            loaded: false,
        }
    }
}

impl<E: Model> From<&u32> for Reference<E> {
    fn from(value: &u32) -> Self {
        Reference {
            result: None,
            id: *value,
            loaded: false,
        }
    }
}
