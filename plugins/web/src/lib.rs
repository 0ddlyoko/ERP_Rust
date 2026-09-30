//! What a browser reaches: the files plugins serve, the bundles they form, and the web client.
//!
//! Installs itself once `base` is there, so every application that has users also serves them a
//! face without anybody asking for it.

use erp::assets::{BundleContribution, StaticFiles, TemplateFiles};
use erp::http::ControllerRegistry;
use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};
use erp::types::field::SingleId;

pub mod controllers;
pub mod models;
pub mod qweb;

include!(concat!(env!("OUT_DIR"), "/static_files.rs"));
include!(concat!(env!("OUT_DIR"), "/template_files.rs"));

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

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Template<_>>();
        model_manager
            .shared_caches
            .register(models::template::BUNDLES_CACHE, &["template"]);
        model_manager
            .shared_caches
            .register(models::template::RESOLVED_CACHE, &["template"]);
        model_manager
            .load_hooks
            .push(models::Template::<SingleId>::on_plugin_loaded);
    }

    fn init_controllers(&self, controllers: &mut ControllerRegistry) {
        controllers.register::<controllers::Home>();
        controllers.register::<controllers::Web>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![include_str!("../data/access.xml")]
    }

    fn static_files(&self) -> StaticFiles {
        STATIC_FILES
    }

    fn template_files(&self) -> TemplateFiles {
        TEMPLATE_FILES
    }

    fn assets(&self) -> Vec<BundleContribution> {
        vec![BundleContribution::new(
            "web.assets_backend",
            &[
                "web/static/src/**/*.css",
                "web/static/src/**/*.js",
                "web/static/src/**/*.xml",
            ],
        )]
    }

    fn imports(&self) -> Vec<(&'static str, &'static str)> {
        vec![("trame", "web/static/lib/trame.js")]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string()]
    }

    fn auto_install(&self) -> bool {
        true
    }
}

code_gen::export_plugin!(WebPlugin {});
