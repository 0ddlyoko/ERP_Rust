//! Purchases: requests for quotation, purchase orders and their vendor bills, with the prices
//! vendors ask.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod models;
pub mod vendor_price;

pub struct PurchasePlugin;

impl Plugin for PurchasePlugin {
    fn name(&self) -> String {
        "purchase".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Requests for quotation and purchase orders, billed as ordered or as received, \
                 with vendor prices."
                    .to_string(),
            ),
            category: Some("Purchases".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            color: Some("#0e7c66".to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::SupplierInfo<_>>();
        model_manager.register_model::<models::ProductPurchase<_>>();
        model_manager.register_model::<models::PurchaseOrder<_>>();
        model_manager.register_model::<models::PurchaseOrderLine<_>>();
        model_manager.register_model::<models::InvoiceLinePurchase<_>>();
        model_manager.register_model::<models::MovePurchase<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/groups.xml"),
            include_str!("../data/access.xml"),
            include_str!("../data/purchase_data.xml"),
            include_str!("../views/purchase_views.xml"),
            include_str!("../views/menus.xml"),
        ]
    }

    fn demo(&self) -> Vec<&'static str> {
        vec![include_str!("../demo/purchase.xml")]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["account".to_string()]
    }
}

code_gen::export_plugin!(PurchasePlugin {});
