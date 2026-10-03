//! The thread of every record: what changed in its tracked fields, and later what people say
//! about it. Installs itself once `base` and `web` are there.

use erp::assets::{BundleContribution, StaticFiles};
use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod models;

include!(concat!(env!("OUT_DIR"), "/static_files.rs"));

pub struct MailPlugin;

impl Plugin for MailPlugin {
    fn name(&self) -> String {
        "mail".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "The thread of every record: changes of its tracked fields.".to_string(),
            ),
            category: Some("Technical".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Message<_>>();
        model_manager.register_model::<models::MessageChange<_>>();
        model_manager.tracking_hooks.push(models::note_changes);
        model_manager.create_hooks.push(models::note_creation);
        model_manager.delete_hooks.push(models::forget_deleted);
    }

    fn static_files(&self) -> StaticFiles {
        STATIC_FILES
    }

    fn assets(&self) -> Vec<BundleContribution> {
        vec![BundleContribution::new(
            "web.assets_backend",
            &[
                "mail/static/src/**/*.css",
                "mail/static/src/**/*.js",
                "mail/static/src/**/*.xml",
            ],
        )]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string(), "web".to_string()]
    }

    fn auto_install(&self) -> bool {
        true
    }
}

code_gen::export_plugin!(MailPlugin {});
