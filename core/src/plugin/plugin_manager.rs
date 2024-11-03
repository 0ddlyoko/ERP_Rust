use std::collections::HashMap;
use std::fmt::Pointer;
use std::fs;
use std::ops::Deref;
use std::path::PathBuf;
use libloading::Error;
use crate::plugin::Plugin;

pub struct PluginManager {
    plugins: HashMap<&'static str, Box<dyn Plugin>>,
}

impl Default for PluginManager {
    fn default() -> Self {
        PluginManager {
            plugins: HashMap::new(),
        }
    }
}

impl PluginManager {

    pub fn load_plugins(&mut self, directory_path: String) -> Result<(), Box<dyn std::error::Error>> {
        let paths = fs::read_dir(directory_path).unwrap();
        for path in paths {
            let path = path.unwrap().path();
            self.add_plugin(&path)?;
        }

        Ok(())
    }

    pub unsafe fn read_plugin_from_file(path: &PathBuf) -> Result<*mut Box<dyn Plugin>, Error> {
        type PluginCreator = unsafe extern "C" fn() -> *mut Box<dyn Plugin>;

        let lib = libloading::Library::new(path)?;
        let func: libloading::Symbol<PluginCreator> = lib.get(b"_create_plugin")?;
        Ok(func())
    }

    fn add_plugin(&mut self, plugin_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            let plugin = Self::read_plugin_from_file(plugin_path)?;
            let mut plugin = Box::from_raw(plugin);
            plugin.init();
            // self.plugins.insert(name, plugin);
        }
        Ok(())
    }

    // unsafe fn main() -> Result<(), Box<dyn std::error::Error>> {
    //     // Chemin vers la bibliothèque dynamique du plugin
    //     let lib = libloading::Library::new("plugins/test_plugin/target/debug/libtest_plugin.so")?;
    // 
    //     // Charge la fonction `_create_plugin` depuis la bibliothèque
    //     let create_plugin: libloading::Symbol<unsafe extern "C" fn() -> *mut Box<dyn Plugin>> = lib.get(b"_create_plugin")?;
    // 
    //     // Appelle la fonction pour obtenir une instance du plugin
    //     let mut plugin = Box::from_raw(create_plugin());
    // 
    //     // Utilise le plugin
    //     plugin.init();
    //     plugin.execute("Hello from main");
    // 
    //     // Libération explicite pour éviter les fuites mémoire
    //     drop(plugin);
    //     Ok(())
    // }

    // fn add_plugin<P>(&mut self, plugin: Box<P>) where P: Plugin + 'static {
    //     let name = plugin.name();
    //     self.plugins.insert(name, plugin);
    // }
}
