//! Sales and inventory together: a confirmed order is delivered from the warehouse, and what is
//! delivered is what the order counts — and invoices, for products invoiced as delivered.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod models;

pub struct SaleStockPlugin;

impl Plugin for SaleStockPlugin {
    fn name(&self) -> String {
        "sale_stock".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Deliveries of confirmed sales orders, and the quantities they deliver."
                    .to_string(),
            ),
            category: Some("Sales".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::StockMoveSale<_>>();
        model_manager.register_model::<models::SaleOrderLineStock<_>>();
        model_manager.register_model::<models::SaleOrderStock<_>>();
        model_manager.register_model::<models::PickingSale<_>>();
        model_manager.register_model::<models::ProductSaleStock<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![include_str!("../views/sale_stock_views.xml")]
    }

    fn demo(&self) -> Vec<&'static str> {
        vec![include_str!("../demo/sale_stock.xml")]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["sale".to_string(), "stock".to_string()]
    }

    fn auto_install(&self) -> bool {
        true
    }
}

code_gen::export_plugin!(SaleStockPlugin {});
