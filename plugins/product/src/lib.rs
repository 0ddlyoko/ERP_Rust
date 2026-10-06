//! Products: what is sold, bought or kept, with their prices, units and categories.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod models;

pub struct ProductPlugin;

impl Plugin for ProductPlugin {
    fn name(&self) -> String {
        "product".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Products and services, their categories, prices and units of measure.".to_string(),
            ),
            category: Some("Sales".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::ProductCategory<_>>();
        model_manager.register_model::<models::Product<_>>();
        model_manager.register_model::<models::UomProduct<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/access.xml"),
            include_str!("../data/product_data.xml"),
            include_str!("../views/product_views.xml"),
        ]
    }

    fn demo(&self) -> Vec<&'static str> {
        vec![include_str!("../demo/products.xml")]
    }

    fn get_depends(&self) -> Vec<String> {
        vec![
            "base".to_string(),
            "web".to_string(),
            "mail".to_string(),
            "uom".to_string(),
        ]
    }
}

code_gen::export_plugin!(ProductPlugin {});
