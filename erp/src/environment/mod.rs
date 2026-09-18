//! The environment: one unit of work, holding a record cache, the model registry and a database
//! connection for the length of a transaction.
//!
//! The implementation is split across submodules by responsibility; they all extend the same
//! [`Environment`] type.

use crate::database::{Database, DatabaseType};
use crate::errors::MaximumRecursionDepthCompute;
use crate::model::{Model, ModelManager};
use erp_cache::{Cache, CacheField, CacheModels};
use erp_search::{LeftTuple, SearchType};
use erp_search_code_gen::make_domain;
use erp_types::cache::{Dirty, Update};
use erp_types::environment::ErasedEnvironment;
use erp_types::field::FieldType;
use erp_types::field::{FieldDepend, FieldReference, FieldReferenceType};
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use uuid::Uuid;

mod compute;
mod create;
mod flush;
mod read;
mod transaction;
mod write;

const MAX_NUMBER_OF_RECURSION: i32 = 1024;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub fn make_cache(model_manager: &ModelManager) -> Cache {
    let mut cache = HashMap::new();
    for model_name in model_manager.get_models().keys() {
        cache.insert(model_name.clone(), CacheModels::new());
    }
    Cache { cache }
}

pub struct Environment<'mm> {
    pub cache: Cache,
    pub model_manager: &'mm ModelManager,
    pub database: DatabaseType,
    closed: bool,
}

impl Drop for Environment<'_> {
    /// Roll back the transaction opened by [`Environment::new`].
    ///
    /// Does nothing once [`Environment::close`] committed: the transaction is already over, so a
    /// rollback here would apply to whatever the connection is reused for next.
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        // We don't care if there is an issue during the rollback of the transaction
        let _ = self.database.rollback_transaction();
    }
}

impl<'mm> Environment<'mm> {
    pub fn new(model_manager: &'mm ModelManager, database: DatabaseType) -> Result<Self> {
        let mut env = Environment {
            cache: make_cache(model_manager),
            model_manager,
            database,
            closed: false,
        };
        env.database.start_transaction()?;
        Ok(env)
    }
}

/// Name used as the identity check behind [`Environment::from_erased`].
const ENVIRONMENT_TYPE_NAME: &str = "erp::environment::Environment";

impl ErasedEnvironment for Environment<'_> {
    fn erased_type_name(&self) -> &'static str {
        ENVIRONMENT_TYPE_NAME
    }
}
