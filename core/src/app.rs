use crate::config::Config;
use crate::environment::Environment;
use crate::model::ModelManager;
use crate::plugin::Plugin;
use crate::plugin::plugin_manager::PluginManager;
type MyResult = Result<(), Box<dyn std::error::Error>>;

pub struct Application {
    config: Config,
    pub model_manager: ModelManager,
    pub plugin_manager: PluginManager,
}

impl Application {
    pub fn new(config: Config) -> Application {
        Application {
            config,
            model_manager: ModelManager::default(),
            plugin_manager: PluginManager::default(),
        }
    }

    pub fn load(&mut self) -> MyResult {
        self.register_plugins()?;
        self.load_plugins()?;
        self.load_models();
        Ok(())
    }

    fn register_plugins(&mut self) -> MyResult {
        self.plugin_manager.register_plugins(&self.config.plugin_path)?;
        Ok(())
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) -> MyResult {
        self.plugin_manager.register_plugin(plugin)
    }

    fn load_plugins(&mut self) -> MyResult {
        // TODO Only load some plugins, and in correct order
        for (_, internal_plugin) in self.plugin_manager.plugins.iter_mut() {
            internal_plugin.plugin.init();
            internal_plugin.plugin.init_models(&mut self.model_manager);
        }

        Ok(())
    }

    pub fn load_plugin(&mut self, plugin_name: &'static str) -> MyResult {
        let plugin = self.plugin_manager.get_plugin_mut(plugin_name).unwrap_or_else(|| panic!("Plugin {} is not registered", plugin_name));
        plugin.plugin.init();
        plugin.plugin.init_models(&mut self.model_manager);

        Ok(())
    }

    fn load_models(&mut self) {
        self.plugin_manager.plugins.iter().for_each(|(_, internal_plugin)| {
            internal_plugin.plugin.init_models(&mut self.model_manager);
        })
    }

    pub fn unload(&mut self) {
        self.plugin_manager.unload();
        self.plugin_manager = PluginManager::default();
        self.model_manager = ModelManager::default();
    }

    fn new_env(&self) -> Environment {
        Environment::new(&self.model_manager)
    }
}
