use crate::config::Config;
use crate::environment::Environment;
use crate::model::ModelManager;
use crate::plugin::plugin_manager::PluginManager;

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

    pub fn load(&mut self) {
        self.load_plugins();
        self.load_models();
    }

    fn load_plugins(&mut self) {
        self.plugin_manager.load_plugins(self.config.plugin_path.clone());
    }

    fn load_models(&mut self) {

    }

    fn new_env(&self) -> Environment {
        Environment::new(&self.model_manager)
    }
}
