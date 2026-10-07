//! `default_get`, `create`, `write` and `delete`, overridable on every model.
//!
//! Every model gets the four at the bottom of its chains, doing what the ORM does; a plugin
//! overrides them like any method of an `#[erp_methods]` block, reaching the implementation below
//! through `sup` — to change the values before, or act on the records after.

use super::{Model, ModelManager};
use crate::environment::Environment;
use erp_types::environment::ErasedEnvironment;
use erp_types::field::MultipleIds;
use erp_types::method::{Receiver, Super};
use erp_types::model::MapOfFields;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub const DEFAULT_GET: &str = "default_get";
pub const CREATE: &str = "create";
pub const WRITE: &str = "write";
pub const DELETE: &str = "delete";

/// What `default_get` takes: the fields whose starting values are asked. It answers those that
/// have one.
pub type DefaultGetArgs = (Vec<String>,);
/// What `create` takes: the values of each record to create. It answers the records created.
pub type CreateArgs = (Vec<MapOfFields>,);
/// What `write` takes: the values to write to every record.
pub type WriteArgs = (MapOfFields,);
/// What `delete` takes: nothing but the records. It answers how many went.
pub type DeleteArgs = ();

/// Put the ORM's own `default_get`, `create`, `write` and `delete` at the bottom of a model's
/// chains, once.
pub(crate) fn register_crud<M>(model_manager: &mut ModelManager)
where
    M: Model<MultipleIds> + 'static,
{
    let model_name = M::_get_model_name();
    let registered = model_manager
        .try_get_model(model_name)
        .is_ok_and(|model| model.methods.get(CREATE).is_some());
    if registered {
        return;
    }
    model_manager.register_method::<DefaultGetArgs, MapOfFields>(
        model_name,
        DEFAULT_GET,
        base_default_get::<M>,
        Receiver::Model,
        "erp",
    );
    model_manager.register_method::<CreateArgs, MultipleIds>(
        model_name,
        CREATE,
        base_create::<M>,
        Receiver::Records,
        "erp",
    );
    model_manager.register_method::<WriteArgs, ()>(
        model_name,
        WRITE,
        base_write::<M>,
        Receiver::Records,
        "erp",
    );
    model_manager.register_method::<DeleteArgs, u32>(
        model_name,
        DELETE,
        base_delete::<M>,
        Receiver::Records,
        "erp",
    );
}

/// The defaults the fields declare: `#[erp(default = …)]`.
fn base_default_get<M>(
    _: MultipleIds,
    env: &mut dyn ErasedEnvironment,
    args: &DefaultGetArgs,
    _: Super<'_, DefaultGetArgs, MapOfFields>,
) -> Result<MapOfFields>
where
    M: Model<MultipleIds>,
{
    let env = Environment::from_erased(env);
    let model = env.model_manager.try_get_model(M::_get_model_name())?;
    let mut defaults = MapOfFields::default();
    for name in &args.0 {
        if let Some(value) = model
            .fields
            .get(name)
            .and_then(|field| field.default_value.clone())
        {
            defaults.insert_field_type(name, value);
        }
    }
    Ok(defaults)
}

fn base_create<M>(
    _: MultipleIds,
    env: &mut dyn ErasedEnvironment,
    args: &CreateArgs,
    _: Super<'_, CreateArgs, MultipleIds>,
) -> Result<MultipleIds>
where
    M: Model<MultipleIds>,
{
    Environment::from_erased(env)._create_new_records(M::_get_model_name(), args.0.clone())
}

fn base_write<M>(
    ids: MultipleIds,
    env: &mut dyn ErasedEnvironment,
    args: &WriteArgs,
    _: Super<'_, WriteArgs, ()>,
) -> Result<()>
where
    M: Model<MultipleIds>,
{
    Environment::from_erased(env).write_records(M::_get_model_name(), &ids, args.0.clone())
}

fn base_delete<M>(
    ids: MultipleIds,
    env: &mut dyn ErasedEnvironment,
    _: &DeleteArgs,
    _: Super<'_, DeleteArgs, u32>,
) -> Result<u32>
where
    M: Model<MultipleIds>,
{
    Environment::from_erased(env).delete_records(M::_get_model_name(), &ids)
}

/// Creating, writing and deleting a model's records from its Rust type, each through the model's
/// overridable method. A trait rather than methods of each struct, so that a struct overriding one
/// of them declares it without a clash.
pub trait ModelVerbs: Model<MultipleIds> + Sized {
    /// Create one record per set of values.
    fn create(values: Vec<MapOfFields>, env: &mut Environment) -> Result<Self> {
        env.create_new_records_from_maps::<Self>(values)
    }

    /// Write the same values to every record of the set.
    fn write(&self, values: MapOfFields, env: &mut Environment) -> Result<()> {
        env.write(Self::_get_model_name(), self.get_id_mode(), values)
    }

    /// Delete these records, and report how many went.
    fn delete(&self, env: &mut Environment) -> Result<u32> {
        env.delete(Self::_get_model_name(), self.get_id_mode())
    }
}

impl<M: Model<MultipleIds>> ModelVerbs for M {}
