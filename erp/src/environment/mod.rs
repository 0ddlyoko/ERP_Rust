//! The environment: one unit of work, holding a record cache, the model registry and a database
//! connection for the length of a transaction.
//!
//! The implementation is split across submodules by responsibility; they all extend the same
//! [`Environment`] type.

use crate::database::{Database, DatabaseType};
use crate::errors::MaximumRecursionDepthCompute;
use crate::model::{Model, ModelManager};
use crate::server_config::ServerConfig;
use erp_cache::{Cache, CacheField, CacheModels};
use erp_search::{LeftTuple, SearchOptions, SearchType};
use erp_search_code_gen::make_domain;
use erp_types::cache::{Dirty, Update};
use erp_types::environment::ErasedEnvironment;
use erp_types::field::FieldType;
use erp_types::field::{FieldDepend, FieldReference, FieldReferenceType};
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::error::Error;
use uuid::Uuid;

mod access;
mod commands;
mod compute;
mod create;
mod delete;
mod flush;
mod method;
mod onchange;
pub use onchange::{LineKey, Onchange, OnchangeLines, VIRTUAL_IDS_FROM, is_virtual};
mod read;
mod required;
mod shared;
mod tracking;
mod transaction;
mod write;

/// Rounds the recompute driver may go through before it declares a computed field unstable.
///
/// Counts turns of a flat loop, not nesting: it catches a compute that keeps dirtying what it
/// just wrote, never a method calling itself.
const MAX_RECOMPUTE_ROUNDS: i32 = 1024;

/// Methods that may be on the stack at once.
///
/// Measured rather than guessed: nesting dies around 1300 frames on a 2 MB thread in a debug
/// build, which is the worst configuration the tests run in. This leaves a margin of six while
/// staying an order of magnitude above any legitimate nesting — a method reaching its own name
/// is a cycle, not a deep call.
const MAX_CALL_DEPTH: usize = 200;

/// Records loaded along with the one asked for, at most, when reading a field of a recordset.
///
/// Enough to turn a loop over a page of records into one query, small enough that a loop over a
/// whole table does not pull it all into memory at the first read.
const PREFETCH_MAX: usize = 1000;

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
    /// What the server was configured with.
    ///
    /// The server's half of the configuration and not the whole of it: a model's code has
    /// business knowing how long a session lasts, and none knowing the database password.
    server_config: &'mm ServerConfig,
    /// Who this unit of work runs on behalf of.
    ///
    /// `None` means nobody in particular: booting, loading plugins, system work. Carried here
    /// rather than passed around because retrofitting it later would touch every signature.
    uid: Option<u32>,
    /// Methods currently running, outermost first.
    ///
    /// Kept as names rather than a depth counter: a real cycle is almost never a method calling
    /// itself, it is two of them calling each other, and only the path shows that.
    call_stack: Vec<(String, String)>,
    computing: Vec<(String, String)>,
    sudo: bool,
    access_memo: access::AccessMemo,
    changed_cached_models: HashSet<String>,
    tracked: tracking::Tracked,
    /// Records a deletion under way removes, by model: their own fields may be emptied, required
    /// or not, and records pointing to them are not held back by them.
    deleting: HashMap<String, HashSet<u32>>,
    /// Records a cascade is about to delete through their model's `delete`, by model: deleted
    /// there as part of the deletion under way, whatever the caller's rights.
    cascading: HashMap<String, HashSet<u32>>,
    maybe_emptied: required::MaybeEmptied,
    /// How many virtual records this unit of work made, for the next one's id.
    /// Plugins this unit of work asks to install once it is committed.
    requested_installs: Vec<String>,
    virtual_count: u32,
    closed: bool,
    /// The external identifiers of the modules a data file names, read once per module while
    /// the file loads; `None` outside of that.
    pub(crate) external_ids: Option<crate::data::ExternalIds>,
    /// The plugins' rows and the parameters, read once while the application loads.
    pub(crate) boot: Option<crate::plugin::BootRecords>,
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
    pub fn new(
        model_manager: &'mm ModelManager,
        server_config: &'mm ServerConfig,
        database: DatabaseType,
    ) -> Result<Self> {
        Self::new_as(model_manager, server_config, database, None)
    }

    /// Same, on behalf of a user.
    pub fn new_as(
        model_manager: &'mm ModelManager,
        server_config: &'mm ServerConfig,
        database: DatabaseType,
        uid: Option<u32>,
    ) -> Result<Self> {
        let mut env = Environment {
            cache: make_cache(model_manager),
            model_manager,
            database,
            server_config,
            uid,
            call_stack: Vec::new(),
            computing: Vec::new(),
            sudo: false,
            access_memo: access::AccessMemo::default(),
            changed_cached_models: HashSet::new(),
            tracked: Default::default(),
            deleting: HashMap::new(),
            cascading: HashMap::new(),
            maybe_emptied: Default::default(),
            requested_installs: Vec::new(),
            virtual_count: 0,
            closed: false,
            external_ids: None,
            boot: None,
        };
        env.database.start_transaction()?;
        Ok(env)
    }

    /// Whether nobody authenticated: no user, or the one a request starts as without a session.
    pub fn is_anonymous(&self) -> bool {
        self.uid.is_none() || self.uid == self.model_manager.identities.default_user()
    }

    /// Who this environment runs on behalf of, if anyone.
    pub fn uid(&self) -> Option<u32> {
        self.uid
    }

    /// What the server was configured with.
    pub fn server_config(&self) -> &ServerConfig {
        self.server_config
    }

    /// Whether this runs as the user every rule lets through.
    pub fn is_root(&self) -> bool {
        self.uid.is_some() && self.uid == self.model_manager.identities.root_user()
    }

    /// The same unit of work, carried on as somebody else.
    ///
    /// An environment rather than a flag to pass around: what changes is who the work is for, and
    /// everything else — the cache, the transaction, what has been written and not yet
    /// committed — is deliberately the same. Two environments would be two transactions, and
    /// reading back what you just wrote would stop working.
    ///
    /// Who it was for comes back when the returned environment is dropped, so a switch cannot
    /// outlive the lines that asked for it. Acting as somebody means acting with their rights, so
    /// a surrounding [`Environment::sudo`] does not carry over.
    pub fn as_user(&mut self, uid: u32) -> AsUser<'_, 'mm> {
        let previous = self.uid;
        let previous_sudo = self.sudo;
        self.uid = Some(uid);
        self.sudo = false;
        AsUser {
            env: self,
            previous,
            previous_sudo,
        }
    }

    /// The same user, with access rights no longer checked.
    ///
    /// Unlike [`Environment::as_root`], who the work is for does not change: a method authorising
    /// its own writes still reads `uid()` as the caller. Needs no root
    /// either, so it works before the plugin naming one has loaded.
    pub fn sudo(&mut self) -> AsUser<'_, 'mm> {
        let previous = self.uid;
        let previous_sudo = self.sudo;
        self.sudo = true;
        AsUser {
            env: self,
            previous,
            previous_sudo,
        }
    }

    /// Run `work` as [`Environment::sudo`] does, and give back what it gives: the sudo lasts as
    /// long as `work`, not a block written around it.
    pub fn sudo_with<R>(&mut self, work: impl FnOnce(&mut Environment<'mm>) -> R) -> R {
        work(&mut self.sudo())
    }

    /// Ask for a plugin to be installed, with what it depends on, once this unit of work is
    /// committed: whoever serves the application installs it then, and serves the application
    /// with it from the next request on. Nothing is asked if the work is rolled back.
    pub fn request_install(&mut self, plugin_name: &str) {
        if !self
            .requested_installs
            .iter()
            .any(|name| name == plugin_name)
        {
            self.requested_installs.push(plugin_name.to_string());
        }
    }

    /// Whether access rights are skipped for this environment.
    pub fn is_sudo(&self) -> bool {
        self.sudo
    }

    /// The same, as the user every rule lets through.
    ///
    /// For work the process does on its own account rather than on a caller's — resolving a
    /// token, running a scheduled job — where what the caller may see is beside the point.
    ///
    /// Fails when no plugin has said which record that is, which is what an application with no
    /// users looks like. Better than quietly carrying on as somebody who is not root.
    pub fn as_root(&mut self) -> Result<AsUser<'_, 'mm>> {
        let root = self.model_manager.identities.root_user().ok_or(
            "No plugin has said which user every rule lets through, so there is no root to act as",
        )?;
        Ok(self.as_user(root))
    }
}

/// An environment running as somebody else, for as long as it is held.
///
/// Reached through [`Environment::as_user`] and [`Environment::as_root`]. It is the environment it
/// came from — it dereferences to it, and shares its cache and its transaction — with one
/// difference, and it puts that difference back on the way out.
pub struct AsUser<'e, 'mm> {
    env: &'e mut Environment<'mm>,
    previous: Option<u32>,
    previous_sudo: bool,
}

impl<'mm> std::ops::Deref for AsUser<'_, 'mm> {
    type Target = Environment<'mm>;

    fn deref(&self) -> &Self::Target {
        self.env
    }
}

impl std::ops::DerefMut for AsUser<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.env
    }
}

impl Drop for AsUser<'_, '_> {
    fn drop(&mut self) {
        self.env.uid = self.previous;
        self.env.sudo = self.previous_sudo;
    }
}

/// Name used as the identity check behind [`Environment::from_erased`].
const ENVIRONMENT_TYPE_NAME: &str = "erp::environment::Environment";

impl ErasedEnvironment for Environment<'_> {
    fn erased_type_name(&self) -> &'static str {
        ENVIRONMENT_TYPE_NAME
    }
}
