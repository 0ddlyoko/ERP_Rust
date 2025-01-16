mod errors;
mod map_of_fields;
mod model_descriptor;
mod model_manager;

pub use errors::*;
pub use map_of_fields::*;
pub use model_descriptor::*;
pub use model_manager::*;
use std::error::Error;

use crate::environment::Environment;
use crate::field::{FieldType, Reference, RequiredFieldEmpty};

pub trait BaseModel {
    fn get_model_name() -> &'static str;
}

pub trait Model: SimplifiedModel + Sized {
    type BaseModel: BaseModel + Sized;

    // Utils

    /// Returns given field of given type.
    /// If error, returns the error
    fn get<'a, E>(&self, field_name: &str, env: &'a mut Environment) -> Result<&'a E, Box<dyn Error>>
    where
        Option<&'a E>: From<&'a FieldType>,
    {
        let model_name = Self::get_model_name();
        let id = self.get_id();
        let result: Option<&FieldType> = env.get_field_value(model_name, field_name, id)?;
        if let Some(result) = result {
            let result: Option<&E> = result.into();

            if let Some(result) = result {
                Ok(result)
            } else {
                Err(RequiredFieldEmpty {
                    model_name: model_name.to_string(),
                    field_name: field_name.to_string(),
                    id,
                }.into())
            }
        } else {
            Err(RequiredFieldEmpty {
                model_name: model_name.to_string(),
                field_name: field_name.to_string(),
                id,
            }.into())
        }
    }

    /// Returns given optional field of given type.
    /// If error, returns the error
    fn get_option<'a, E>(&self, field_name: &str, env: &'a mut Environment) -> Result<Option<&'a E>, Box<dyn std::error::Error>>
    where
        Option<&'a E>: From<&'a FieldType>,
        Self: Sized + Model,
    {
        let model_name = Self::get_model_name();
        let id = self.get_id();
        let result: Option<&FieldType> = env.get_field_value(model_name, field_name, id)?;
        Ok(result.and_then(|result| result.into()))
    }

    /// Returns given optional reference field.
    /// If error, returns the error
    fn get_reference<M, BM>(&self, field_name: &str, env: &mut Environment) -> Result<Option<M>, Box<dyn std::error::Error>>
    where
        BM: BaseModel,
        M: Model<BaseModel=BM>,
        Self: Sized + Model,
    // Option<erp::field::Reference<BM>>: From<&erp::field::FieldType>,
    {
        let model_name = Self::get_model_name();
        let id = self.get_id();
        let result: Option<&FieldType> = env.get_field_value(model_name, field_name, id)?;
        let reference: Option<Reference<BM>> = result.and_then(|result| result.into());
        if let Some(mut reference) = reference {
            reference.get::<M>(env)
        } else {
            Ok(None)
        }
    }

    /// Changes the value of given field to given value
    fn set<E>(&self, field_name: &str, value: E, env: &mut Environment) -> Result<(), Box<dyn std::error::Error>>
    where
        E: Into<FieldType>,
        Self: Sized + Model,
    {
        let model_name = Self::get_model_name();
        let id = self.get_id();
        env.save_field_value(model_name, field_name, id, value)
    }

    /// Changes the value of given field to given optional value
    fn set_option<E>(&self, field_name: &str, value: Option<E>, env: &mut Environment) -> Result<(), Box<dyn std::error::Error>>
    where
        E: Into<FieldType>,
        Self: Sized + Model,
    {
        let model_name = Self::get_model_name();
        let id = self.get_id();
        env.save_option_field_value(model_name, field_name, id, value)
    }

    /// Changes the value of given field to given reference
    fn set_reference<E>(&self, field_name: &str, value: Reference<E>, env: &mut Environment) -> Result<(), Box<dyn std::error::Error>>
    where
        E: BaseModel,
        Self: Sized + Model,
    {
        let model_name = Self::get_model_name();
        let id = self.get_id();
        env.save_field_value(model_name, field_name, id, value)
    }
}

pub trait SimplifiedModel {

    fn get_model_name() -> &'static str
    where
        Self: Sized + Model {
        <Self as Model>::BaseModel::get_model_name()
    }

    fn get_model_descriptor() -> ModelDescriptor
    where
        Self: Sized;

    /// Returns the id of the current record
    fn get_id(&self) -> u32;
    /// Create an instance of this model with given list of data
    /// TODO Data not needed anymore
    fn create_model(id: u32, data: MapOfFields) -> Self
    where
        Self: Sized;

    /// Call computed method
    fn call_compute_method(
        &mut self,
        field_name: &str,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error>>;
}

// impl<T: Model> SimplifiedModel for T {
//     fn get_model_name() -> &'static str
//     where
//         Self: Sized
//     {
//         T::get_model_name()
//     }
//
//     fn get_model_descriptor() -> ModelDescriptor
//     where
//         Self: Sized
//     {
//         T::get_model_descriptor()
//     }
//
//     fn get_id(&self) -> u32 {
//         T::get_id(self)
//     }
//
//     fn get_data(&self) -> MapOfFields {
//         T::get_data(self)
//     }
//
//     fn create_model(id: u32, data: MapOfFields) -> Self
//     where
//         Self: Sized
//     {
//         T::create_model(id, data)
//     }
//
//     fn call_compute_method(&mut self, field_name: &str, env: &mut Environment) -> Result<(), Box<dyn Error>> {
//         T::call_compute_method(self, field_name, env)
//     }
// }
