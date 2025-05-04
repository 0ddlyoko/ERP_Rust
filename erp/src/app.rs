use erp_database::cache::CacheDatabase;
use erp_database::Database;
use erp_database::postgres::PostgresDatabase;
use crate::config::Config;
use crate::environment::Environment;
use crate::model::ModelManager;
use crate::plugin::InternalPluginState::Installed;
use crate::plugin::Plugin;
use crate::plugin::PluginManager;

type EmptyResult = Result<(), Box<dyn std::error::Error>>;

pub struct Application {
    config: Config,
    pub model_manager: ModelManager,
    pub plugin_manager: PluginManager,
    pub database: Box<dyn Database>,
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
                    database: Box::new(database),
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
            database: Box::new(database),
        }
    }

    pub fn load(&mut self) -> EmptyResult {
        self.register_plugins()?;
        self.initialize_db()?;
        self.load_plugins()?;
        self.load_models();
        Ok(())
    }

    fn register_plugins(&mut self) -> EmptyResult {
        self.plugin_manager
            .register_plugins(&self.config.plugin_path)?;
        Ok(())
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) -> EmptyResult {
        self.plugin_manager.register_plugin(plugin)
    }

    fn initialize_db(&mut self) -> EmptyResult {
        if !self.database.is_installed()? {
            self.database.initialize()?;
        }
        Ok(())
    }

    fn load_plugins(&mut self) -> EmptyResult {
        // TODO Only load plugins if they are installed
        let ordered_depends: Vec<String> = self
            .plugin_manager
            ._get_ordered_dependencies()?
            .iter()
            .map(|&str| str.to_string())
            .collect();

        for plugin_name in ordered_depends.iter() {
            self._load_plugin(plugin_name.as_str())?;
        }

        Ok(())
    }

    pub fn load_plugin(&mut self, plugin_name: &str) -> EmptyResult {
        // Only detect if there is a recursion along all the plugins. We don't care about the result
        self.plugin_manager._get_ordered_dependencies()?;

        self._load_plugin(plugin_name)
    }

    /// Load given plugin and all plugins that the given one depends.
    /// Do not check if there is a recursion between plugins.
    fn _load_plugin(&mut self, plugin_name: &str) -> EmptyResult {
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

        let plugin = self.plugin_manager.load_plugin(plugin_name)?;
        plugin.plugin.init_models(&mut self.model_manager);

        Ok(())
    }

    fn load_models(&mut self) {
        self.plugin_manager
            .plugins
            .iter()
            .for_each(|(_, internal_plugin)| {
                internal_plugin.plugin.init_models(&mut self.model_manager);
            })
    }

    pub fn unload(&mut self) {
        self.plugin_manager.unload();
        self.plugin_manager = PluginManager::default();
        self.model_manager = ModelManager::default();
    }

    pub fn new_env(&self) -> Environment {
        Environment::new(&self.model_manager, self.database.as_ref())
    }
}
