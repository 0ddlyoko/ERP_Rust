//! Units of measure: what quantities are counted in, grouped by what they measure, and how one
//! converts into another of the same kind.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod conversion;
pub mod models;

pub struct UomPlugin;

impl Plugin for UomPlugin {
    fn name(&self) -> String {
        "uom".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Units of measure: units, weights, lengths, volumes, and conversions between them."
                    .to_string(),
            ),
            category: Some("Inventory".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::UomCategory<_>>();
        model_manager.register_model::<models::Uom<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/access.xml"),
            include_str!("../data/uom_data.xml"),
            include_str!("../views/uom_views.xml"),
        ]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string(), "web".to_string()]
    }
}

code_gen::export_plugin!(UomPlugin {});
