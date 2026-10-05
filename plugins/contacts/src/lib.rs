//! The address book as an application of its own: its menu, over the contacts `base` declares.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub struct ContactsPlugin;

impl Plugin for ContactsPlugin {
    fn name(&self) -> String {
        "contacts".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "The address book: customers, suppliers, and everyone else, people and companies."
                    .to_string(),
            ),
            category: Some("Sales".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, _model_manager: &mut ModelManager) {}

    fn data(&self) -> Vec<&'static str> {
        vec![include_str!("../views/menus.xml")]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string(), "web".to_string(), "mail".to_string()]
    }
}

code_gen::export_plugin!(ContactsPlugin {});
