use crate::config::Config;
use crate::database::cache::CacheDatabase;
use crate::database::postgres::{ConnectionPool, PostgresDatabase};
use crate::database::{Database, DatabaseType};
use crate::environment::Environment;
use crate::model::ModelManager;
use crate::plugin::InternalPluginState::Installed;
use crate::plugin::Plugin;
use crate::plugin::PluginManager;
use crate::util::dependency::CircularDependencyError;
use std::error::Error;
use std::sync::OnceLock;
use std::time::Instant;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Which installed plugins have their data loaded again although their version did not change.
///
/// A plugin's data is loaded when it is installed and when its version changes; asking for an
/// update is how a change made without bumping the version reaches the database.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DataUpdate {
    #[default]
    Nothing,
    All,
    Only(Vec<String>),
}

/// What the command line asks of a start: plugins to install, and plugins whose data to load
/// again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchArgs {
    pub install: Vec<String>,
    pub update: DataUpdate,
}

impl LaunchArgs {
    /// Read `-i`/`--install` (plugin names separated by commas) and `-u`/`--update` (`all`, or
    /// plugin names) from the command line, each as often as wanted.
    ///
    /// Anything else is refused rather than ignored, so a mistyped flag is not an install or an
    /// update that silently did not happen.
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let usage =
            "Usage: [-i|--install <plugin>[,<plugin>...]] [-u|--update all|<plugin>[,<plugin>...]]";
        let mut launch = LaunchArgs::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let (flag, value) = match arg.split_once('=') {
                Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_string())),
                _ => (arg.as_str(), None),
            };
            let install = match flag {
                "-i" | "--install" => true,
                "-u" | "--update" => false,
                _ => return Err(format!("Unknown argument {arg}. {usage}").into()),
            };
            let value = match value {
                Some(value) => value,
                None => args
                    .next()
                    .ok_or_else(|| format!("{flag} needs a value. {usage}"))?,
            };
            let names: Vec<String> = value
                .split(',')
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect();
            if install {
                launch.install.extend(names);
            } else {
                launch.update = launch.update.and(names);
            }
        }
        Ok(launch)
    }
}

impl DataUpdate {
    /// This update, and the plugins named as well: `all` among them updates everything.
    fn and(self, names: Vec<String>) -> Self {
        match (self, names.iter().any(|name| name == "all")) {
            (DataUpdate::All, _) | (_, true) => DataUpdate::All,
            (DataUpdate::Only(mut before), false) => {
                before.extend(names);
                DataUpdate::Only(before)
            }
            (DataUpdate::Nothing, false) => DataUpdate::Only(names),
        }
    }

    fn includes(&self, plugin_name: &str) -> bool {
        match self {
            DataUpdate::Nothing => false,
            DataUpdate::All => true,
            DataUpdate::Only(names) => names.iter().any(|name| name == plugin_name),
        }
    }
}

pub struct Application {
    config: Config,
    pub model_manager: ModelManager,
    pub plugin_manager: PluginManager,
    pub is_test: bool,
    pub cache_db: CacheDatabase,
    /// Opened on first use rather than at construction, so an application that never reaches the
    /// database — a test, a `--help` — never tries to.
    pool: OnceLock<ConnectionPool>,
    data_update: DataUpdate,
    /// While the application loads, what checks every plugin at once runs only once, at the end.
    checks_deferred: bool,
    /// The schema as the plugins loading so far left it, handed from one to the next so it is
    /// read once; forgotten once they are loaded, as other processes may change it.
    schema_state: Option<crate::database::postgres::SchemaState>,
    install: Vec<String>,
    signing_secret: OnceLock<String>,
}

impl Application {
    /// Create a new instance of this application with given config
    pub fn new(config: Config) -> Application {
        Application {
            config,
            model_manager: ModelManager::default(),
            plugin_manager: PluginManager::default(),
            is_test: false,
            cache_db: CacheDatabase::default(),
            pool: OnceLock::new(),
            data_update: DataUpdate::default(),
            checks_deferred: false,
            schema_state: None,
            install: Vec::new(),
            signing_secret: OnceLock::new(),
        }
    }

    /// A new application on the same configuration and database, signing with the same secret,
    /// not loaded yet: what replaces this one once plugins are installed, so that sessions and
    /// tokens signed before still hold.
    pub fn successor(&self) -> Application {
        let successor = Application {
            config: self.config.clone(),
            model_manager: ModelManager::default(),
            plugin_manager: PluginManager::default(),
            is_test: self.is_test,
            cache_db: self.cache_db.clone(),
            pool: OnceLock::new(),
            data_update: DataUpdate::default(),
            checks_deferred: false,
            schema_state: None,
            install: Vec::new(),
            signing_secret: OnceLock::new(),
        };
        let _ = successor
            .signing_secret
            .set(self.signing_secret().to_string());
        successor
    }

    /// Create a new test instance of this application.
    /// Database used is a cache database, so saved in memory.
    /// Creating new environment instances of this Application will create separated memory database, so
    ///  it's safe to perform parallel operations on multiples test applications
    pub fn new_test() -> Application {
        Application {
            config: Config::default(),
            model_manager: ModelManager::default(),
            plugin_manager: PluginManager::default(),
            is_test: true,
            cache_db: CacheDatabase::default(),
            pool: OnceLock::new(),
            data_update: DataUpdate::default(),
            checks_deferred: false,
            schema_state: None,
            install: Vec::new(),
            signing_secret: OnceLock::new(),
        }
    }

    /// Create a new connection to the database
    /// Open a new, independent connection to the configured database.
    /// The connection pool, opened the first time anything asks for one.
    fn pool(&self) -> Result<&ConnectionPool> {
        if let Some(pool) = self.pool.get() {
            return Ok(pool);
        }
        let pool = ConnectionPool::new(&self.config.database);
        // Another thread may have won the race; either pool is as good, so the loser's is
        // dropped and the winner's used.
        let _ = self.pool.set(pool);
        Ok(self.pool.get().expect("just set"))
    }

    /// How many database connections exist right now, for whoever is watching the load.
    ///
    /// `None` before anything has asked for one, and for an application that never will.
    pub fn pool_size(&self) -> Option<usize> {
        self.pool.get().map(ConnectionPool::open)
    }

    /// Replace the configuration, for a test that needs different settings on a test
    /// application.
    pub fn set_config(&mut self, config: Config) {
        self.config = config;
        self.signing_secret = OnceLock::new();
    }

    /// What signs the tokens the server hands out: the configured secret, or one made up the
    /// first time it is needed.
    pub fn signing_secret(&self) -> &str {
        self.signing_secret.get_or_init(|| match &self.config.server.secret {
            Some(secret) => secret.clone(),
            None => {
                if !self.is_test {
                    tracing::warn!(
                        "No server secret is configured: one is made up, and the tokens it signs \
                         stop working when the server restarts"
                    );
                }
                erp_types::field::generate_secret()
            }
        })
    }

    /// How many requests may be served at once, `0` meaning no bound.
    ///
    /// Read by whatever schedules requests rather than enforced here: waiting for a turn should
    /// not cost a thread, and only the scheduler knows how to wait cheaply.
    pub fn max_concurrent_requests(&self) -> usize {
        self.config.server.max_concurrent_requests
    }

    /// What the server should listen on.
    pub fn server_config(&self) -> &crate::server_config::ServerConfig {
        &self.config.server
    }

    /// How many connections were asked whether they were still alive.
    pub fn pool_revalidations(&self) -> Option<usize> {
        self.pool.get().map(ConnectionPool::revalidations)
    }

    /// How many connections were found dead and thrown away.
    pub fn pool_discarded(&self) -> Option<usize> {
        self.pool.get().map(ConnectionPool::discarded)
    }

    pub fn create_new_database(&self) -> Result<DatabaseType> {
        Ok(if self.is_test {
            DatabaseType::Cache(self.cache_db.connect())
        } else {
            DatabaseType::Postgres(Box::new(PostgresDatabase::connect(
                self.pool()?,
                &self.config.database.schema,
                self.model_manager.tables(),
            )?))
        })
    }

    /// Load the plugins installed, and those asked for; how long it took and how many SQL
    /// statements it ran are logged once done, and for each plugin as it loads.
    pub fn load(&mut self) -> Result<()> {
        let started = Instant::now();
        crate::request_log::start();
        self.checks_deferred = true;
        self.register_plugins()?;
        self.initialize_db()?;
        self.load_base_plugin()?;
        self.record_registered_plugins()?;
        self.load_plugins()?;
        self.auto_install_plugins()?;
        for name in self.install.clone() {
            if self.plugin_manager.get_plugin(&name).is_none() {
                return Err(format!("Cannot install {name}: no such plugin").into());
            }
            if !self.plugin_manager.is_installed(&name) {
                tracing::info!(plugin = %name, "Installing, as asked");
                self.load_plugin(&name)?;
            }
        }
        if let DataUpdate::Only(names) = &self.data_update {
            for name in names {
                if !self.plugin_manager.is_installed(name) {
                    return Err(format!("Cannot update {name}: it is not installed").into());
                }
            }
        }
        self.checks_deferred = false;
        self.release_orphan_columns()?;
        self.schema_state = None;
        self.run_checks()?;
        let done = crate::request_log::current();
        tracing::info!(
            plugins = self.model_manager.loaded_plugins.len(),
            ms = started.elapsed().as_millis(),
            queries = done.queries,
            sql_ms = done.sql_time.as_millis(),
            "Plugins loaded"
        );
        Ok(())
    }

    /// Say which plugins the next load installs, with their dependencies, unless they already are.
    pub fn set_install(&mut self, names: Vec<String>) {
        self.install = names;
    }

    /// Say which plugins get their data loaded again at the next load, whatever their version.
    pub fn set_data_update(&mut self, update: DataUpdate) {
        self.data_update = update;
    }

    /// Record every registered plugin that is not loaded, so the database lists what could be
    /// installed as well as what is.
    ///
    /// A row already saying installed keeps saying so: the plugin is loaded right after, and
    /// knowing it again is not uninstalling it.
    pub fn record_registered_plugins(&mut self) -> Result<()> {
        let mut env = Environment::new(
            &self.model_manager,
            &self.config.server,
            self.create_new_database()?,
        )?;
        for (name, plugin) in &self.plugin_manager.plugins {
            if plugin.state != Installed {
                crate::plugin::record_plugin(&mut env, name, &plugin.plugin.info(), false)?;
            }
        }
        env.close()
    }

    fn register_plugins(&mut self) -> Result<()> {
        self.plugin_manager
            .register_plugins(&self.config.plugin_path)?;
        Ok(())
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<()> {
        self.plugin_manager.register_plugin(plugin)
    }

    fn initialize_db(&mut self) -> Result<()> {
        let mut database = self.create_new_database()?;
        if !database.is_installed()? {
            database.initialize()?;
        }
        Ok(())
    }

    /// Only load plugin "base"
    fn load_base_plugin(&mut self) -> Result<()> {
        // Only detect if there is a recursion along all the plugins. We don't care about the result
        self.plugin_manager
            ._get_ordered_dependencies_of_all_plugins()?;

        self._load_plugin("base")
    }

    /// Load all plugins, except "base"
    ///
    /// If you want to load "base" plugin, please call load_base_plugin
    fn load_plugins(&mut self) -> Result<()> {
        // Only detect if there is a recursion along all the plugins. We don't care about the result
        self.plugin_manager
            ._get_ordered_dependencies_of_all_plugins()?;

        // `base` stays in the list although it is already loaded: the plugins depending on it
        // need it there to be ordered, and loading it again does nothing.
        let mut database = self.create_new_database()?;
        let plugins = database.get_installed_plugins()?;

        // Vec<String> => Vec<&String>
        let plugins = plugins.iter().collect::<Vec<_>>();

        let ordered_depends: Vec<&str> = self.plugin_manager._get_ordered_dependencies(plugins)?;

        for plugin_name in ordered_depends.iter() {
            self._load_plugin(plugin_name)?;
        }

        Ok(())
    }

    /// Install a plugin, its dependencies, and whatever installs itself once they are there.
    pub fn load_plugin(&mut self, plugin_name: &str) -> Result<()> {
        self._load_plugin(plugin_name)?;
        self.auto_install_plugins()?;
        if !self.checks_deferred {
            self.schema_state = None;
            self.run_checks()?;
        }
        Ok(())
    }

    /// Free the columns of fields gone from `NOT NULL`, every plugin being loaded: before, a field
    /// a plugin still to load adds would look gone.
    fn release_orphan_columns(&mut self) -> Result<()> {
        let mut database = self.create_new_database()?;
        if let DatabaseType::Postgres(postgres) = &mut database {
            postgres.schema_state = self.schema_state.take();
        }
        for model in self.model_manager.get_models().values() {
            database.release_orphan_columns(model)?;
        }
        Ok(())
    }

    /// Check at once what every plugin brings: the access rules, and what plugins ask to check
    /// ([`ModelManager::check_hooks`]).
    fn run_checks(&mut self) -> Result<()> {
        let mut env = Environment::new(
            &self.model_manager,
            &self.config.server,
            self.create_new_database()?,
        )?;
        if let Some(check) = env.model_manager.access.source().map(|source| source.check) {
            check(&mut env)?;
        }
        for hook in env.model_manager.check_hooks.clone() {
            hook(&mut env)?;
        }
        env.close()
    }

    /// Install every plugin marked auto-install whose dependencies are all installed.
    ///
    /// Goes round until nothing more installs, so one auto-installed plugin can complete the
    /// dependencies of another. Candidates of one round go in name order, so the same plugins
    /// always install in the same order. Returns the plugins installed, in that order.
    pub fn auto_install_plugins(&mut self) -> Result<Vec<String>> {
        let mut installed = Vec::new();
        loop {
            let mut ready: Vec<String> = self
                .plugin_manager
                .plugins
                .iter()
                .filter(|(_, plugin)| plugin.state != Installed && plugin.plugin.auto_install())
                .filter(|(_, plugin)| {
                    plugin
                        .depends
                        .iter()
                        .all(|depend| self.plugin_manager.is_installed(depend))
                })
                .map(|(name, _)| name.clone())
                .collect();
            if ready.is_empty() {
                return Ok(installed);
            }
            ready.sort();
            for name in ready {
                tracing::info!(
                    plugin = %name,
                    "Installing on its own: its dependencies are installed"
                );
                self._load_plugin(&name)?;
                installed.push(name);
            }
        }
    }

    /// Load given plugin and all plugins that the given one depends.
    fn _load_plugin(&mut self, plugin_name: &str) -> Result<()> {
        self.load_plugin_within(plugin_name, &mut Vec::new())
    }

    /// Load a plugin after its dependencies, `loading` holding the chain that led here.
    ///
    /// A plugin is only marked installed once its dependencies are, so a cycle would recurse
    /// until the stack overflows and takes the process with it. Finding a plugin already in the
    /// chain refuses the load instead, naming the cycle.
    fn load_plugin_within(&mut self, plugin_name: &str, loading: &mut Vec<String>) -> Result<()> {
        let plugin = self
            .plugin_manager
            .get_plugin(plugin_name)
            .unwrap_or_else(|| panic!("Plugin {} is not registered", plugin_name));
        if plugin.state == Installed {
            return Ok(());
        }
        if let Some(start) = loading.iter().position(|name| name == plugin_name) {
            let mut cycle = loading[start..].to_vec();
            cycle.push(plugin_name.to_string());
            return Err(CircularDependencyError {
                plugin_name: plugin_name.to_string(),
                cycle,
            }
            .into());
        }
        let update_asked = self.data_update.includes(plugin_name);
        let depends: Vec<_> = plugin.depends.to_vec();
        loading.push(plugin_name.to_string());
        for depend in depends {
            self.load_plugin_within(depend.as_str(), loading)?;
        }
        loading.pop();
        let started = Instant::now();
        let before = crate::request_log::current();

        // Opened before borrowing the plugin mutably: the connection is owned, so it does not
        // keep `self` borrowed afterwards.
        let database = self.create_new_database()?;
        let plugin = &mut self.plugin_manager.load_plugin(plugin_name)?.plugin;

        plugin.pre_init();
        self.model_manager
            .loaded_plugins
            .push(plugin_name.to_string());
        self.model_manager.shared_caches.forget_all();
        self.model_manager.current_plugin_loading = Some(plugin_name.to_string());
        plugin.init_models(&mut self.model_manager);
        self.model_manager.post_register();
        self.model_manager.current_plugin_loading = None;
        self.model_manager.controllers.current_plugin_loading = Some(plugin_name.to_string());
        plugin.init_controllers(&mut self.model_manager.controllers);
        self.model_manager.controllers.current_plugin_loading = None;
        self.model_manager
            .assets
            .register(plugin_name, plugin.static_files(), plugin.assets());
        self.model_manager.assets.register_imports(plugin.imports());
        self.model_manager
            .assets
            .register_templates(plugin_name, plugin.template_files());

        // Bring the schema in line with what this plugin declared. It runs after
        // `post_register`, so relational links are complete, and in dependency order, so a plugin
        // extending another's model finds the base columns already there.
        let mut database = database;
        if let DatabaseType::Postgres(postgres) = &mut database {
            postgres.schema_state = self.schema_state.take();
        }
        let model_names: Vec<String> = self
            .model_manager
            .get_all_models_for_plugin(plugin_name)
            .iter()
            .map(|model| model.name.clone())
            .collect();
        // A computed field whose column has just appeared has to be worked out for the records
        // that were already there: nothing else would ever fill it.
        let mut to_fill: Vec<(String, String)> = Vec::new();
        for model_name in &model_names {
            let model = self.model_manager.try_get_model(model_name)?;
            for column in database.sync_model(model)? {
                if model.get_internal_field(&column).compute.is_some() {
                    to_fill.push((model_name.clone(), column));
                }
            }
        }
        // A second pass, because a relation table references two model tables and the model
        // declaring it is not necessarily synchronised last.
        for model_name in &model_names {
            let model = self.model_manager.try_get_model(model_name)?;
            database.sync_constraints(model)?;
            database.sync_indexes(model)?;
        }
        if let DatabaseType::Postgres(postgres) = &mut database {
            self.schema_state = postgres.schema_state.take();
        }

        // Data is loaded before `post_init`, so a plugin finds its own records in place by the
        // time its code runs.
        let data = plugin.data();
        self.model_manager
            .demo
            .insert(plugin_name.to_string(), plugin.demo());
        let info = plugin.info();
        let mut env = Environment::new(&self.model_manager, &self.config.server, database)?;
        env.external_ids = Some(crate::data::ExternalIds::default());
        env.savepoint(|env| {
            for (model_name, field_name) in &to_fill {
                env.fill_stored_field(model_name, field_name)?;
            }
            if should_load_data(env, plugin_name, &info, update_asked)? {
                for document in &data {
                    crate::data::load(env, plugin_name, document)?;
                }
            }
            plugin.post_init(env)?;
            for hook in env.model_manager.load_hooks.clone() {
                hook(env, plugin_name)?;
            }
            crate::plugin::record_plugin(env, plugin_name, &info, true)?;
            crate::plugin::demo::load_for(env, plugin_name)
        })?;
        env.close()?;

        let after = crate::request_log::current();
        tracing::info!(
            plugin = %plugin_name,
            ms = started.elapsed().as_millis(),
            queries = after.queries - before.queries,
            sql_ms = (after.sql_time - before.sql_time).as_millis(),
            "Plugin loaded"
        );
        Ok(())
    }

    /// Tear this application down.
    ///
    /// The model registry is cleared before the plugins: it holds the function pointers of every
    /// overridable method and the `TypeId`s keying them, all originating from the plugin dylibs,
    /// which dangle once those libraries are unloaded.
    pub fn unload(mut self) {
        self.model_manager = ModelManager::default();
        self.plugin_manager.unload();
        self.plugin_manager = PluginManager::default();
    }

    /// Open a new environment, with its own database connection and transaction.
    ///
    /// Takes `&self`, so any number of environments can be alive at once.
    /// Opens for whoever a caller is before authenticating, which a plugin names. `None` until
    /// one has, which is what booting looks like.
    pub fn new_env(&self) -> Result<Environment<'_>> {
        self.new_env_as_option(self.model_manager.identities.default_user())
    }

    /// Open a new environment on behalf of a user.
    /// Same, for a caller that may or may not be anyone in particular.
    pub fn new_env_as_option(&self, uid: Option<u32>) -> Result<Environment<'_>> {
        Environment::new_as(
            &self.model_manager,
            &self.config.server,
            self.create_new_database()?,
            uid,
        )
    }

    pub fn new_env_as(&self, uid: u32) -> Result<Environment<'_>> {
        Environment::new_as(
            &self.model_manager,
            &self.config.server,
            self.create_new_database()?,
            Some(uid),
        )
    }
}

/// Whether a plugin's data files are loaded this time.
///
/// On install, when the version changed since it was installed, and when an update was asked
/// for; otherwise the database already holds what the files say, and loading them again would
/// only overwrite what users changed since. The schema and `post_init` run either way: they
/// follow the code, not the data.
fn should_load_data(
    env: &mut Environment,
    plugin_name: &str,
    info: &crate::plugin::PluginInfo,
    update_asked: bool,
) -> Result<bool> {
    let reason = match crate::plugin::installed_version(env, plugin_name)? {
        _ if update_asked => "an update was asked for",
        None => "it is being installed",
        Some(installed) if installed != info.version => "its version changed",
        Some(_) => {
            tracing::debug!(plugin = %plugin_name, "Data left as it is: same version");
            return Ok(false);
        }
    };
    tracing::info!(plugin = %plugin_name, "Loading data: {reason}");
    Ok(true)
}
