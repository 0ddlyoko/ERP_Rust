//! Inventory: warehouses and locations, receipts, deliveries and internal transfers, stock on
//! hand, physical inventory, and what the stock is worth.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod models;
pub mod valuation;

pub struct StockPlugin;

impl Plugin for StockPlugin {
    fn name(&self) -> String {
        "stock".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Warehouses, receipts, deliveries, transfers, stock on hand, inventory counts \
                 and valuation (standard, average, FIFO)."
                    .to_string(),
            ),
            category: Some("Inventory".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Location<_>>();
        model_manager.register_model::<models::Warehouse<_>>();
        model_manager.register_model::<models::PickingType<_>>();
        model_manager.register_model::<models::Picking<_>>();
        model_manager.register_model::<models::StockMove<_>>();
        model_manager.register_model::<models::Quant<_>>();
        model_manager.register_model::<models::ValuationLayer<_>>();
        model_manager.register_model::<models::ProductCategoryStock<_>>();
        model_manager.register_model::<models::ProductStock<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/groups.xml"),
            include_str!("../data/access.xml"),
            include_str!("../data/stock_data.xml"),
            include_str!("../views/stock_views.xml"),
            include_str!("../views/menus.xml"),
        ]
    }

    fn get_depends(&self) -> Vec<String> {
        vec![
            "base".to_string(),
            "web".to_string(),
            "mail".to_string(),
            "contacts".to_string(),
            "currency".to_string(),
            "sequence".to_string(),
            "product".to_string(),
        ]
    }
}

code_gen::export_plugin!(StockPlugin {});
