//! Inventory and accounting together: what a move adds to the stock's value, or takes from it,
//! is booked — perpetual valuation, on the accounts of the product's category.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod models;

pub struct StockAccountPlugin;

impl Plugin for StockAccountPlugin {
    fn name(&self) -> String {
        "stock_account".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Perpetual inventory valuation: receipts and deliveries booked on the stock \
                 accounts of their product's category."
                    .to_string(),
            ),
            category: Some("Accounting".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::ProductCategoryStockAccount<_>>();
        model_manager.register_model::<models::StockMoveStockAccount<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/stock_account_data.xml"),
            include_str!("../views/stock_account_views.xml"),
        ]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["stock".to_string(), "account".to_string()]
    }

    fn auto_install(&self) -> bool {
        true
    }
}

code_gen::export_plugin!(StockAccountPlugin {});
