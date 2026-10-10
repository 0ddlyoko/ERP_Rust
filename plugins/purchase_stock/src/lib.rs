//! Purchases and inventory together: a confirmed purchase order is received into the warehouse,
//! valued at its price, and what is received is what the order counts — and bills, for
//! products billed as received.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod models;

pub struct PurchaseStockPlugin;

impl Plugin for PurchaseStockPlugin {
    fn name(&self) -> String {
        "purchase_stock".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Receipts of confirmed purchase orders, valued at the price paid, and the \
                 quantities they receive."
                    .to_string(),
            ),
            category: Some("Purchases".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::StockMovePurchaseStock<_>>();
        model_manager.register_model::<models::PurchaseOrderLinePurchaseStock<_>>();
        model_manager.register_model::<models::PurchaseOrderPurchaseStock<_>>();
        model_manager.register_model::<models::StockPickingPurchaseStock<_>>();
        model_manager.register_model::<models::ProductPurchaseStock<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![include_str!("../views/purchase_stock_views.xml")]
    }

    fn demo(&self) -> Vec<&'static str> {
        vec![include_str!("../demo/purchase_stock.xml")]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["purchase".to_string(), "stock".to_string()]
    }

    fn auto_install(&self) -> bool {
        true
    }
}

code_gen::export_plugin!(PurchaseStockPlugin {});
