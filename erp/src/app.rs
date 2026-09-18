use crate::config::Config;
use crate::database::cache::CacheDatabase;
use crate::database::postgres::PostgresDatabase;
use crate::database::{Database, DatabaseType};
use crate::environment::Environment;
use crate::model::ModelManager;
use crate::plugin::InternalPluginState::Installed;
use crate::plugin::Plugin;
use crate::plugin::PluginManager;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub struct Application {
    config: Config,
    pub model_manager: ModelManager,
    pub plugin_manager: PluginManager,
    pub is_test: bool,
    pub cache_db: CacheDatabase,
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
        }
    }

    /// Create a new connection to the database
    /// Open a new, independent connection to the configured database.
    pub fn create_new_database(&self) -> Result<DatabaseType> {
        Ok(if self.is_test {
            DatabaseType::Cache(self.cache_db.connect())
        } else {
            DatabaseType::Postgres(Box::new(PostgresDatabase::connect(&self.config.database)?))
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

        // TODO Get all registered models, to update the database
        let _registered_models = self.model_manager.get_all_models_for_plugin(plugin_name);

        let mut env = Environment::new(&self.model_manager, database)?;
        env.savepoint(|env| plugin.post_init(env))?;
        env.close()?;

        Ok(())
    }

    /// Tear this application down.
    ///
    /// The model registry is cleared before the plugins: it holds `computed_method` function
    /// pointers and `TypeId`s originating from the plugin dylibs, which dangle once those
    /// libraries are unloaded.
    pub fn unload(mut self) {
        self.model_manager = ModelManager::default();
        self.plugin_manager.unload();
        self.plugin_manager = PluginManager::default();
    }

    /// Open a new environment, with its own database connection and transaction.
    ///
    /// Takes `&self`, so any number of environments can be alive at once.
    pub fn new_env(&self) -> Result<Environment<'_>> {
        Environment::new(&self.model_manager, self.create_new_database()?)
    }
}
