use crate::config::Config;
use crate::database::cache::CacheDatabase;
use crate::database::postgres::{ConnectionPool, PostgresDatabase};
use crate::database::{Database, DatabaseType};
use crate::environment::Environment;
use crate::model::ModelManager;
use crate::plugin::InternalPluginState::Installed;
use crate::plugin::Plugin;
use crate::plugin::PluginManager;
use std::error::Error;
use std::sync::OnceLock;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub struct Application {
    config: Config,
    pub model_manager: ModelManager,
    pub plugin_manager: PluginManager,
    pub is_test: bool,
    pub cache_db: CacheDatabase,
    /// Opened on first use rather than at construction, so an application that never reaches the
    /// database — a test, a `--help` — never tries to.
    pool: OnceLock<ConnectionPool>,
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
        }
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

    pub fn load(&mut self) -> Result<()> {
        self.register_plugins()?;
        self.initialize_db()?;
        self.load_base_plugin()?;
        self.load_plugins()?;
        Ok(())
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

        self.load_plugin("base")
    }

    /// Load all plugins, except "base"
    ///
    /// If you want to load "base" plugin, please call load_base_plugin
    fn load_plugins(&mut self) -> Result<()> {
        // Only detect if there is a recursion along all the plugins. We don't care about the result
        self.plugin_manager
            ._get_ordered_dependencies_of_all_plugins()?;

        let mut database = self.create_new_database()?;
        let mut plugins = database.get_installed_plugins()?;
        plugins.retain(|plugin_name| plugin_name != "base");

        // Vec<String> => Vec<&String>
        let plugins = plugins.iter().collect::<Vec<_>>();

        let ordered_depends: Vec<&str> = self.plugin_manager._get_ordered_dependencies(plugins)?;

        for plugin_name in ordered_depends.iter() {
            self.load_plugin(plugin_name)?;
        }

        Ok(())
    }

    pub fn load_plugin(&mut self, plugin_name: &str) -> Result<()> {
        self._load_plugin(plugin_name)
    }

    /// Load given plugin and all plugins that the given one depends.
    /// Do not check if there is a recursion between plugins.
    fn _load_plugin(&mut self, plugin_name: &str) -> Result<()> {
        let plugin = self
            .plugin_manager
            .get_plugin(plugin_name)
            .unwrap_or_else(|| panic!("Plugin {} is not registered", plugin_name));
        if plugin.state == Installed {
            return Ok(());
        }
        let depends: Vec<_> = plugin.depends.to_vec();
        for depend in depends {
            self._load_plugin(depend.as_str())?;
        }

        // Opened before borrowing the plugin mutably: the connection is owned, so it does not
        // keep `self` borrowed afterwards.
        let database = self.create_new_database()?;
        let plugin = &mut self.plugin_manager.load_plugin(plugin_name)?.plugin;

        plugin.pre_init();
        self.model_manager.current_plugin_loading = Some(plugin_name.to_string());
        plugin.init_models(&mut self.model_manager);
        self.model_manager.post_register();
        self.model_manager.current_plugin_loading = None;

        // Bring the schema in line with what this plugin declared. It runs after
        // `post_register`, so relational links are complete, and in dependency order, so a plugin
        // extending another's model finds the base columns already there.
        let mut database = database;
        let model_names: Vec<String> = self
            .model_manager
            .get_all_models_for_plugin(plugin_name)
            .iter()
            .map(|model| model.name.clone())
            .collect();
        for model_name in &model_names {
            let model = self.model_manager.try_get_model(model_name)?;
            database.sync_model(model)?;
        }
        // A second pass, because a relation table references two model tables and the model
        // declaring it is not necessarily synchronised last.
        for model_name in &model_names {
            let model = self.model_manager.try_get_model(model_name)?;
            database.sync_constraints(model)?;
        }

        // Data is loaded before `post_init`, so a plugin finds its own records in place by the
        // time its code runs.
        let data = plugin.data();
        let mut env = Environment::new(&self.model_manager, &self.config.server, database)?;
        env.savepoint(|env| {
            for document in &data {
                crate::data::load(env, plugin_name, document)?;
            }
            plugin.post_init(env)
        })?;
        env.close()?;

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
