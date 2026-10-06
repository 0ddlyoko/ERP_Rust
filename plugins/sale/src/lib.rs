//! Sales: quotations, sales orders and their invoicing, with pricelists and discounts.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod models;
pub mod pricing;

pub struct SalePlugin;

impl Plugin for SalePlugin {
    fn name(&self) -> String {
        "sale".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Quotations and sales orders, invoiced as ordered or as delivered, with \
                 pricelists and discounts."
                    .to_string(),
            ),
            category: Some("Sales".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            color: Some("#4b3fe0".to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Pricelist<_>>();
        model_manager.register_model::<models::PricelistItem<_>>();
        model_manager.register_model::<models::ContactSale<_>>();
        model_manager.register_model::<models::ProductSale<_>>();
        model_manager.register_model::<models::SaleOrder<_>>();
        model_manager.register_model::<models::SaleOrderLine<_>>();
        model_manager.register_model::<models::InvoiceLineSale<_>>();
        model_manager.register_model::<models::MoveSale<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/groups.xml"),
            include_str!("../data/access.xml"),
            include_str!("../data/sale_data.xml"),
            include_str!("../views/pricelist_views.xml"),
            include_str!("../views/sale_views.xml"),
            include_str!("../views/menus.xml"),
        ]
    }

    fn demo(&self) -> Vec<&'static str> {
        vec![include_str!("../demo/sale.xml")]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["account".to_string()]
    }
}

code_gen::export_plugin!(SalePlugin {});
