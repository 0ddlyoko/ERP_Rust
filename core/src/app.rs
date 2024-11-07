use crate::config::Config;
use crate::environment::Environment;
use crate::model::ModelManager;
use crate::plugin::internal_plugin::InternalPluginState::Installed;
use crate::plugin::plugin_manager::PluginManager;
use crate::plugin::Plugin;

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
        let ordered_depends = self.plugin_manager._get_ordered_dependencies()?;

        for plugin_name in ordered_depends.iter() {
            self._load_plugin(plugin_name)?;
        }

        Ok(())
    }

    pub fn load_plugin(&mut self, plugin_name: &'static str) -> MyResult {
        // Only detect if there is a recursion along all the plugins. We don't care about the result
        self.plugin_manager._get_ordered_dependencies()?;

        self._load_plugin(plugin_name)
    }

    /// Load given plugin and all plugins that the given one depends.
    /// Do not check if there is a recursion between plugins.
    fn _load_plugin(&mut self, plugin_name: &'static str) -> MyResult {
        let plugin = self.plugin_manager.get_plugin(plugin_name).unwrap_or_else(|| panic!("Plugin {} is not registered", plugin_name));
        if plugin.state == Installed {
            return Ok(())
        }
        let depends: Vec<_> = plugin.depends.to_vec();
        for depend in depends {
            self._load_plugin(depend)?;
        }

        let plugin = self.plugin_manager.load_plugin(plugin_name)?;
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
