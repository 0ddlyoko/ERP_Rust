//! What a browser reaches: the files plugins serve, the bundles they form, and the web client.
//!
//! Installs itself once `base` is there, so every application that has users also serves them a
//! face without anybody asking for it.

use erp::assets::StaticFiles;
use erp::http::ControllerRegistry;
use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod controllers;

include!(concat!(env!("OUT_DIR"), "/static_files.rs"));

pub struct WebPlugin;

impl Plugin for WebPlugin {
    fn name(&self) -> String {
        "web".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "The web client, and the files and bundles plugins serve.".to_string(),
            ),
            category: Some("Technical".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, _model_manager: &mut ModelManager) {}

    fn init_controllers(&self, controllers: &mut ControllerRegistry) {
        controllers.register::<controllers::Home>();
        controllers.register::<controllers::Web>();
    }

    fn static_files(&self) -> StaticFiles {
        STATIC_FILES
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string()]
    }

    fn auto_install(&self) -> bool {
        true
    }
}

code_gen::export_plugin!(WebPlugin {});
