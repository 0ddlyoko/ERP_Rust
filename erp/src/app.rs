use std::error::Error;
use crate::config::Config;
use crate::database::cache::CacheDatabase;
use crate::database::{Database, DatabaseType};
use crate::database::postgres::PostgresDatabase;
use crate::environment::Environment;
use crate::model::ModelManager;
use crate::plugin::InternalPluginState::Installed;
use crate::plugin::Plugin;
use crate::plugin::PluginManager;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub struct Application {
    config: Config,
    pub model_manager: ModelManager,
    pub plugin_manager: PluginManager,
    pub database: DatabaseType,
}

impl Application {
    /// Create a new instance of this application with given config
    pub fn new(config: Config) -> Application {
        let database = PostgresDatabase::connect(&config.database);
        match database {
            Ok(database) => {
                Application {
                    config,
                    model_manager: ModelManager::default(),
                    plugin_manager: PluginManager::default(),
                    database: database.into(),
                }
            }
            Err(err) => {
                eprintln!("Error while connecting to database: {}", err);
                panic!("Error while connecting to database, exiting application");
            }
        }
    }

    /// Create a new test instance of this application.
    /// Database used is a cache database, so saved in memory.
    /// Creating new test instances of this Application will create separated memory database, so
    ///  it's safe to perform parallel operations on multiples test applications
    pub fn new_test() -> Application {
        let database = CacheDatabase::connect(&Default::default()).unwrap();
        Application {
            config: Config::default(),
            model_manager: ModelManager::default(),
            plugin_manager: PluginManager::default(),
            database: database.into(),
        }
    }

    pub fn load(&mut self) -> Result<()> {
        self.register_plugins()?;
        self.initialize_db()?;
        self.load_base_plugin()?;
        self.load_plugins()?;
        Ok(())
    }

    fn register_plugins(&mut self) -> Result<()> {
        self.plugin_manager.register_plugins(&self.config.plugin_path)?;
        Ok(())
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<()> {
        self.plugin_manager.register_plugin(plugin)
    }

    fn initialize_db(&mut self) -> Result<()> {
        if !self.database.is_installed()? {
            self.database.initialize()?;
        }
        Ok(())
    }

    /// Only load plugin "base"
    fn load_base_plugin(&mut self) -> Result<()> {
        // Only detect if there is a recursion along all the plugins. We don't care about the result
        self.plugin_manager._get_ordered_dependencies_of_all_plugins()?;

        self.load_plugin("base")
    }

    /// Load all plugins, except "base"
    ///
    /// If you want to load "base" plugin, please call load_base_plugin
    fn load_plugins(&mut self) -> Result<()> {
        // Only detect if there is a recursion along all the plugins. We don't care about the result
        self.plugin_manager._get_ordered_dependencies_of_all_plugins()?;

        let mut plugins = self.database.get_installed_plugins()?;
        plugins.retain(|plugin_name| plugin_name != "base");

        // Vec<String> => Vec<&String>
        let plugins = plugins.iter().collect::<Vec<_>>();

        let ordered_depends: Vec<&str> = self
            .plugin_manager
            ._get_ordered_dependencies(plugins)?;

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

        let plugin = &mut self.plugin_manager.load_plugin(plugin_name)?.plugin;

        plugin.pre_init();
        self.model_manager.current_plugin_loading = Some(plugin_name.to_string());
        plugin.init_models(&mut self.model_manager);
        self.model_manager.post_register();
        self.model_manager.current_plugin_loading = None;

        // TODO Get all registered models, to update the database
        let _registered_models = self.model_manager.get_all_models_for_plugin(plugin_name);

        // Well it looks like this works, but not the call to new_env ...
        let mut env = Environment::new(&self.model_manager, &mut self.database);
        env.database.start_transaction()?;
        env.savepoint(|env| {
            plugin.post_init(env)
        })?;
        env.database.commit_transaction()?;

        Ok(())
    }

    pub fn unload(mut self) {
        self.plugin_manager.unload();
        self.plugin_manager = PluginManager::default();
        self.model_manager = ModelManager::default();
    }

    pub fn new_env(&mut self) -> Environment {
        Environment::new(&self.model_manager, &mut self.database)
    }
}
