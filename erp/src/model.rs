mod errors;
mod iterator;
mod map_of_fields;
mod model_descriptor;
mod model_manager;

pub use errors::*;
pub use iterator::*;
pub use map_of_fields::*;
pub use model_descriptor::*;
pub use model_manager::*;

use crate::environment::Environment;
use crate::field::{FieldType, IdMode, MultipleIds, Reference, RequiredFieldEmpty, SingleId};
use std::error::Error;

pub trait BaseModel {
    fn get_model_name() -> &'static str;
}

/// Common version of the model (that will be implemented for each model)
/// This common trait contains methods that could be applied for both SingleId & MultipleIds structs
pub trait CommonModel<Mode: IdMode> {
    /// Get this model name
    fn get_model_name() -> &'static str
    where
        Self: Sized + Model<Mode> {
        <Self as Model<Mode>>::BaseModel::get_model_name()
    }

    /// Get a descriptor that represent this model
    /// This method should only be called at startup, to load the model
    fn get_model_descriptor() -> ModelDescriptor where Self: Sized;

    /// Get the current id of this model
    fn get_id_mode(&self) -> &Mode;

    /// Create a new instance of this model with given id
    fn create_instance(id: Mode) -> Self where Self: Sized;

    fn create_single_id_instance(id: SingleId) -> Box<dyn CommonModel<SingleId>> where Self: Sized;
    fn create_multiple_ids_instance(id: MultipleIds) -> Box<dyn CommonModel<MultipleIds>> where Self: Sized;
}

pub trait Model<Mode: IdMode>: CommonModel<Mode> {
    type BaseModel: BaseModel;

    /// Call given computed method
    /// This method will only be called with a Model<MultipleIds>, not with Model<SingleId>
    /// So, when you implement this method in Model<SingleId>, you can return Ok(()), as this method
    ///  will never be called on SingleId
    fn call_compute_method(
        &self,
        field_name: &str,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error>>;
}

impl<BM: BaseModel> dyn Model<SingleId, BaseModel=BM> {

    /// Returns given field of given type.
    ///
    /// If error, returns the error
    pub fn get<'a, E>(&self, field_name: &str, env: &'a mut Environment) -> Result<&'a E, Box<dyn Error>>
    where
        Option<&'a E>: From<&'a FieldType>,
    {
        let model_name = <Self as Model<SingleId>>::BaseModel::get_model_name();
        let id: &SingleId = self.get_id_mode();
        let result: Option<&FieldType> = env.get_field_value(model_name, field_name, id)?;
        if let Some(result) = result {
            let result: Option<&E> = result.into();

            if let Some(result) = result {
                Ok(result)
            } else {
                Err(RequiredFieldEmpty {
                    model_name: model_name.to_string(),
                    field_name: field_name.to_string(),
                    id: id.get_id(),
                }.into())
            }
        } else {
            Err(RequiredFieldEmpty {
                model_name: model_name.to_string(),
                field_name: field_name.to_string(),
                id: id.get_id(),
            }.into())
        }
    }

    /// Returns given optional field of given type.
    ///
    /// If error, returns the error
    pub fn get_option<'a, E>(&self, field_name: &str, env: &'a mut Environment) -> Result<Option<&'a E>, Box<dyn std::error::Error>>
    where
        Option<&'a E>: From<&'a FieldType>,
    {
        let model_name = <Self as Model<SingleId>>::BaseModel::get_model_name();
        let id = self.get_id_mode();
        let result: Option<&FieldType> = env.get_field_value(model_name, field_name, id)?;
        Ok(result.and_then(|result| result.into()))
    }

    /// Returns given optional reference field.
    ///
    /// If error, returns the error
    pub fn get_reference<M, BM2>(&self, field_name: &str, env: &mut Environment) -> Result<Option<M>, Box<dyn std::error::Error>>
    where
        M: Model<SingleId, BaseModel=BM2>,
        BM2: BaseModel,
    {
        let model_name = <Self as Model<SingleId>>::BaseModel::get_model_name();
        let id = self.get_id_mode();
        let result: Option<&FieldType> = env.get_field_value(model_name, field_name, id)?;
        let reference: Option<Reference<BM2, SingleId>> = result.and_then(|result| result.into());
        Ok(reference.map(|r| r.get::<M>()))
    }
}

impl<BM: BaseModel> dyn Model<MultipleIds, BaseModel=BM> {

    /// Returns given field of given type.
    ///
    /// If error, returns the error
    pub fn gets<'a, E>(&self, field_name: &str, env: &'a mut Environment) -> Result<Vec<&'a E>, Box<dyn Error>>
    where
        Option<&'a E>: From<&'a FieldType>,
    {
        let model_name = <Self as Model<MultipleIds>>::BaseModel::get_model_name();
        let ids: &MultipleIds = self.get_id_mode();
        let result: Vec<Option<&FieldType>> = env.get_fields_value(model_name, field_name, ids)?;
        result.iter().enumerate().map(|(idx, res)| {
            if let Some(res) = *res {
                let result: Option<&E> = res.into();
                if let Some(result) = result {
                    Ok(result)
                } else {
                    Err(RequiredFieldEmpty {
                        model_name: model_name.to_string(),
                        field_name: field_name.to_string(),
                        id: *ids.get_id_at(idx),
                    }.into())
                }
            } else {
                Err(RequiredFieldEmpty {
                    model_name: model_name.to_string(),
                    field_name: field_name.to_string(),
                    id: *ids.get_id_at(idx),
                }.into())
            }
        }).collect()
    }

    /// Returns given optional field of given type.
    ///
    /// If error, returns the error
    pub fn get_options<'a, E>(&self, field_name: &str, env: &'a mut Environment) -> Result<Vec<Option<&'a E>>, Box<dyn std::error::Error>>
    where
        Option<&'a E>: From<&'a FieldType>,
    {
        let model_name = <Self as Model<MultipleIds>>::BaseModel::get_model_name();
        let id: &MultipleIds = self.get_id_mode();
        let result: Vec<Option<&FieldType>> = env.get_fields_value(model_name, field_name, id)?;
        Ok(result.iter().map(|res| res.and_then(|res| res.into())).collect())
    }
}

impl<Mode: IdMode, BM: BaseModel> dyn Model<Mode, BaseModel=BM> {

    /// Returns given optional references field.
    ///
    /// If error, returns the error
    pub fn get_references<'a, M, BM2>(&self, field_name: &str, env: &'a mut Environment) -> Result<M, Box<dyn std::error::Error>>
    where
        M: Model<MultipleIds, BaseModel=BM2>,
        BM2: BaseModel,
        &'a Mode: IntoIterator<Item = SingleId>,
        Mode: 'a,
    {
        let model_name = <Self as Model<Mode>>::BaseModel::get_model_name();
        let ids = self.get_id_mode();
        let result: Vec<Option<&FieldType>> = env.get_fields_value(model_name, field_name, ids)?;
        let ids: Vec<u32> = result.iter().flat_map(|field_type| {
            if let Some(field_type) = field_type {
                match field_type {
                    FieldType::Ref(id) => vec![*id],
                    FieldType::Refs(ids) => ids.clone(),
                    _ => vec![]
                }
            } else {
                vec![]
            }
        }).collect();
        // Remove duplicated ids
        let mut reference: Reference<BM2, MultipleIds> = ids.into();
        reference.remove_dup();
        Ok(reference.get_multiple::<M>())
    }

    /// Changes the value of given field to given value
    pub fn set<E>(&self, field_name: &str, value: E, env: &mut Environment) -> Result<(), Box<dyn std::error::Error>>
    where
        E: Into<FieldType>,
        for<'a> &'a Mode: IntoIterator<Item = SingleId>,
    {
        let model_name = <Self as Model<Mode>>::BaseModel::get_model_name();
        let id_mode = self.get_id_mode();
        env.save_field_value(model_name, field_name, id_mode, value)
    }

    /// Changes the value of given field to given optional value
    pub fn set_option<E>(&self, field_name: &str, value: Option<E>, env: &mut Environment) -> Result<(), Box<dyn std::error::Error>>
    where
        E: Into<FieldType>,
        for<'a> &'a Mode: IntoIterator<Item = SingleId>,
    {
        let model_name = <Self as Model<Mode>>::BaseModel::get_model_name();
        let id_mode = self.get_id_mode();
        env.save_option_field_value(model_name, field_name, id_mode, value)
    }

    /// Changes the value of given field to given reference
    pub fn set_reference<E>(&self, field_name: &str, value: Reference<E, SingleId>, env: &mut Environment) -> Result<(), Box<dyn std::error::Error>>
    where
        E: BaseModel,
        for<'a> &'a Mode: IntoIterator<Item = SingleId>,
    {
        let model_name = <Self as Model<Mode>>::BaseModel::get_model_name();
        let id_mode = self.get_id_mode();
        env.save_field_value(model_name, field_name, id_mode, value)
    }

    /// Changes the value of given field to given reference
    pub fn set_references<E>(&self, field_name: &str, value: Reference<E, MultipleIds>, env: &mut Environment) -> Result<(), Box<dyn std::error::Error>>
    where
        E: BaseModel,
        for<'a> &'a Mode: IntoIterator<Item = SingleId>,
    {
        let model_name = <Self as Model<Mode>>::BaseModel::get_model_name();
        let id_mode = self.get_id_mode();
        env.save_field_value(model_name, field_name, id_mode, value)
    }

    /// Convert this model into another one, but from the same base
    pub fn convert<TO>(&self) -> TO
    where
        TO: Model<Mode, BaseModel=BM>,
    {
        TO::create_instance(self.get_id_mode().clone())
    }
}
