mod crud;
mod errors;
mod iterator;
mod model_manager;
mod rpc;
mod selections;

pub(crate) use crud::register_crud;
pub use crud::{
    CREATE, CreateArgs, DEFAULT_GET, DELETE, DefaultGetArgs, DeleteArgs, ModelVerbs, WRITE,
    WriteArgs,
};
pub use errors::*;
pub use iterator::*;
pub use model_manager::*;
pub use rpc::*;
pub use selections::*;

use crate::environment::Environment;
use erp_types::field::{EmptyValue, FieldType, Reference};
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::{BaseModel, CommonModel};
use std::error::Error;
use std::sync::Arc;

// We need to make another trait here to be able to implement methods, as we are in another crate.
pub trait Model<Mode: IdMode>: CommonModel<Mode> {}

impl<BM: BaseModel> dyn Model<SingleId, BaseModel = BM> {
    /// Returns the given field of the given type; its type's default when it holds none, as
    /// on an empty record.
    ///
    /// If error, returns the error
    pub fn get<'a, E>(
        &self,
        field_name: &str,
        env: &'a mut Environment,
    ) -> Result<&'a E, Box<dyn Error + Send + Sync>>
    where
        &'a FieldType: Into<Option<&'a E>>,
        E: EmptyValue,
    {
        let model_name = Self::get_model_name();
        let id: &SingleId = self.get_id_mode();
        let result: Option<&FieldType> = env.get_field_value(model_name, field_name, id)?;
        let value: Option<&'a E> = result.and_then(|result| result.into());
        Ok(value.unwrap_or_else(|| E::empty()))
    }

    /// Returns the given optional field of the given type.
    ///
    /// If error, returns the error
    pub fn get_option<'a, E>(
        &self,
        field_name: &str,
        env: &'a mut Environment,
    ) -> Result<Option<&'a E>, Box<dyn Error + Send + Sync>>
    where
        &'a FieldType: Into<Option<&'a E>>,
    {
        let model_name = Self::get_model_name();
        let id = self.get_id_mode();
        let result: Option<&FieldType> = env.get_field_value(model_name, field_name, id)?;
        Ok(result.and_then(|result| result.into()))
    }

    /// Returns the record the given many2one points to: an empty one when it points nowhere.
    ///
    /// It remembers what the rest of the recordset points to, as Odoo's prefetching does:
    /// reading a field of one of them loads it for all.
    ///
    /// If error, returns the error
    pub fn get_reference<M, BM2>(
        &self,
        field_name: &str,
        env: &mut Environment,
    ) -> Result<M, Box<dyn Error + Send + Sync>>
    where
        M: Model<SingleId, BaseModel = BM2>,
        BM2: BaseModel,
    {
        let model_name = Self::get_model_name();
        let id = self.get_id_mode();
        let target = match env.get_field_value(model_name, field_name, id)? {
            Some(FieldType::Ref(target)) => *target,
            _ => return Ok(Reference::<BM2, SingleId>::from(SingleId::empty()).get::<M>()),
        };
        let mut targets: Vec<u32> = id
            .prefetch_ids()
            .iter()
            .filter_map(|other| {
                match env
                    .cache
                    .get_field_from_cache(model_name, field_name, *other)
                {
                    Some(FieldType::Ref(target)) => Some(*target),
                    _ => None,
                }
            })
            .collect();
        targets.sort_unstable();
        targets.dedup();
        let target = SingleId::within(target, Arc::from(targets));
        Ok(Reference::<BM2, SingleId>::from(target).get::<M>())
    }
}

impl<BM: BaseModel> dyn Model<MultipleIds, BaseModel = BM> {
    /// Returns the given field of the given type; its type's default where it holds none.
    ///
    /// If error, returns the error
    pub fn gets<'a, E>(
        &self,
        field_name: &str,
        env: &'a mut Environment,
    ) -> Result<Vec<&'a E>, Box<dyn Error + Send + Sync>>
    where
        &'a FieldType: Into<Option<&'a E>>,
        E: EmptyValue,
    {
        let model_name = Self::get_model_name();
        let ids: &MultipleIds = self.get_id_mode();
        let result: Vec<Option<&FieldType>> = env.get_fields_value(model_name, field_name, ids)?;
        Ok(result
            .into_iter()
            .map(|result| {
                let value: Option<&'a E> = result.and_then(|result| result.into());
                value.unwrap_or_else(|| E::empty())
            })
            .collect())
    }

    /// Returns given optional field of the given type.
    ///
    /// If error, returns the error
    pub fn get_options<'a, E>(
        &self,
        field_name: &str,
        env: &'a mut Environment,
    ) -> Result<Vec<Option<&'a E>>, Box<dyn Error + Send + Sync>>
    where
        &'a FieldType: Into<Option<&'a E>>,
    {
        let model_name = Self::get_model_name();
        let id: &MultipleIds = self.get_id_mode();
        let result: Vec<Option<&FieldType>> = env.get_fields_value(model_name, field_name, id)?;
        Ok(result
            .iter()
            .map(|res| res.and_then(|res| res.into()))
            .collect())
    }
}

impl<Mode: IdMode, BM: BaseModel> dyn Model<Mode, BaseModel = BM> {
    pub fn get_model_name() -> &'static str {
        <Self as CommonModel<Mode>>::BaseModel::_get_model_name()
    }

    /// Returns given optional references field.
    ///
    /// If error, returns the error
    pub fn get_references<M, BM2>(
        &self,
        field_name: &str,
        env: &mut Environment,
    ) -> Result<M, Box<dyn Error + Send + Sync>>
    where
        M: Model<MultipleIds, BaseModel = BM2>,
        BM2: BaseModel,
    {
        let model_name = Self::get_model_name();
        let ids = self.get_id_mode();
        let result: Vec<Option<&FieldType>> = env.get_fields_value(model_name, field_name, ids)?;
        let ids: Vec<u32> = result
            .iter()
            .flat_map(|field_type| {
                if let Some(field_type) = field_type {
                    match field_type {
                        FieldType::Ref(id) => vec![*id],
                        FieldType::Refs(ids) => ids.clone(),
                        _ => vec![],
                    }
                } else {
                    vec![]
                }
            })
            .collect();
        // Remove duplicated ids
        let mut reference: Reference<BM2, MultipleIds> = ids.into();
        reference.remove_dup();
        Ok(reference.get_multiple::<M>())
    }

    /// Changes the value of the given field to the given value
    pub fn set<E>(
        &self,
        field_name: &str,
        value: E,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        E: Into<FieldType>,
    {
        let model_name = Self::get_model_name();
        let id_mode = self.get_id_mode();
        env.save_value_to_cache(model_name, field_name, id_mode, value)
    }

    /// Changes the value of the given field to the given optional value
    pub fn set_option<E>(
        &self,
        field_name: &str,
        value: Option<E>,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        E: Into<FieldType>,
    {
        let model_name = Self::get_model_name();
        let id_mode = self.get_id_mode();
        env.save_option_to_cache(model_name, field_name, id_mode, value)
    }

    /// Changes the value of the given field to the given reference
    pub fn set_reference<E>(
        &self,
        field_name: &str,
        value: Reference<E, SingleId>,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        E: BaseModel,
    {
        let model_name = Self::get_model_name();
        let id_mode = self.get_id_mode();
        env.save_value_to_cache(model_name, field_name, id_mode, value)
    }

    /// Changes the value of the given field to the given reference
    pub fn set_references<E>(
        &self,
        field_name: &str,
        value: Reference<E, MultipleIds>,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        E: BaseModel,
    {
        let model_name = Self::get_model_name();
        let id_mode = self.get_id_mode();
        env.save_value_to_cache(model_name, field_name, id_mode, value)
    }

    /// Convert this model into another one, but from the same base
    pub fn convert<TO>(&self) -> TO
    where
        TO: Model<Mode, BaseModel = BM>,
    {
        TO::create_instance(self.get_id_mode().clone())
    }
}

/// Marks a struct whose `#[erp(methods)]` says an `#[erp_methods]` block exists.
///
/// Implemented by the derive only when the attribute is there, so that the block can assert it
/// and fail to compile when the two disagree. [`HasMethods`] cannot play that role: it is
/// implemented either way, empty when there is nothing to register.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has an #[erp_methods] block but its struct is missing #[erp(methods)]",
    label = "add `methods` to the #[erp(...)] attribute on the struct",
    note = "without it, registering the model would not register these methods, and calling one \
            would fail at runtime instead of here"
)]
pub trait DeclaresMethods {}

/// The overridable methods one `#[erp_methods]` block declares, on `Model<MultipleIds>` or
/// `Model<SingleId>`.
///
/// Implemented by the block. The derive registers both kinds of block of its model through
/// [`MethodsProbe`], which skips a kind the model has no block of.
pub trait MethodBlock {
    fn register_block(model_manager: &mut ModelManager, plugin_name: &str);
}

/// Registers the methods of `T`'s `#[erp_methods]` block if there is one, and does nothing
/// otherwise — without the derive having to know which blocks a model has.
///
/// `(&MethodsProbe::<T>::new()).register(..)` reaches [`RegisterBlock`] when `T` implements
/// [`MethodBlock`]; otherwise only [`NoBlock`], one reference further, applies.
#[doc(hidden)]
pub struct MethodsProbe<T>(std::marker::PhantomData<T>);

impl<T> MethodsProbe<T> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

#[doc(hidden)]
pub trait RegisterBlock {
    fn register(&self, model_manager: &mut ModelManager, plugin_name: &str);
}

impl<T: MethodBlock> RegisterBlock for MethodsProbe<T> {
    fn register(&self, model_manager: &mut ModelManager, plugin_name: &str) {
        T::register_block(model_manager, plugin_name);
    }
}

#[doc(hidden)]
pub trait NoBlock {
    fn register(&self, _model_manager: &mut ModelManager, _plugin_name: &str) {}
}

impl<T> NoBlock for &MethodsProbe<T> {}

/// Declares the overridable methods a struct contributes.
///
/// Generated by `#[erp::methods]`. Kept apart from [`Model`] because a derive macro sees only the
/// struct, never the `impl` block where the methods live, so the two cannot be emitted together.
pub trait HasMethods {
    /// Add this struct's implementations to the registry, under the plugin loading it.
    fn register_methods(model_manager: &mut ModelManager, plugin_name: &str);
}
