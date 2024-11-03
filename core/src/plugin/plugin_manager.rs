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
            unsafe {
                // let result = Self::read_plugin_from_file(&path)?;
                self.add_plugin(&path)?;
            }
        }

        Ok(())
    }

    pub unsafe fn read_plugin_from_file(path: &PathBuf) -> Result<Box<dyn Plugin>, Error> {
        type PluginCreator = unsafe extern fn() -> Box<dyn Plugin>;
        
        let lib = libloading::Library::new(path)?;
        let func: libloading::Symbol<PluginCreator> = lib.get(b"_create_plugin")?;
        Ok(func())
    }

    fn add_plugin(&mut self, plugin_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            let plugin = Self::read_plugin_from_file(plugin_path)?;
            let name = plugin.name();
            self.plugins.insert(name, plugin);
        }
        Ok(())
    }

    // fn add_plugin<P>(&mut self, plugin: Box<P>) where P: Plugin + 'static {
    //     let name = plugin.name();
    //     self.plugins.insert(name, plugin);
    // }
}
