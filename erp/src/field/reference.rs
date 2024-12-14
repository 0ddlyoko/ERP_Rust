use crate::cache::errors::RecordNotFoundError;
use crate::environment::Environment;
use crate::model::{BaseModel, Model};
use std::error::Error;
use std::marker::PhantomData;

pub struct Reference<E: BaseModel> {
    pub id: u32,
    pub _phantom_data: PhantomData<E>,
}

impl<E: BaseModel> Reference<E> {

    /// Retrieves the instance of this ref
    /// mut is needed as we could load the given ref into the cache.
    /// If there is any errors while loading the record, the error is returned.
    /// If the record is of type "RecordNotFoundError", Ok(None) is returned instead
    pub fn get<F>(&mut self, env: &mut Environment) -> Result<Option<F>, Box<dyn Error>>
    where
        F: Model<BaseModel=E> {
        let result = env.get_record::<F>(self.id);
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

impl<E: BaseModel> From<u32> for Reference<E> {
    fn from(value: u32) -> Self {
        Reference {
            id: value,
            _phantom_data: Default::default(),
        }
    }
}

impl<E: BaseModel> From<&u32> for Reference<E> {
    fn from(value: &u32) -> Self {
        Reference {
            id: *value,
            _phantom_data: Default::default(),
        }
    }
}
