//! Numbering of documents: `INV/2026/00042`, one series per kind of document, starting over each
//! year or month when asked.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod format;
pub mod models;

pub struct SequencePlugin;

impl Plugin for SequencePlugin {
    fn name(&self) -> String {
        "sequence".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Numbering of documents, by series, restarting each year or month.".to_string(),
            ),
            category: Some("Technical".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Sequence<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/access.xml"),
            include_str!("../views/sequence_views.xml"),
        ]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string(), "web".to_string()]
    }
}

code_gen::export_plugin!(SequencePlugin {});
