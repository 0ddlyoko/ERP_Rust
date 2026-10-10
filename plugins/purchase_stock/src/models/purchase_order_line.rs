use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference};
use stock::models::BaseStockMove;

/// The moves receiving an order line.
#[derive(Model)]
#[erp(id = "purchase_order_line")]
#[erp(derived_model = "purchase::models")]
#[allow(dead_code)]
pub struct PurchaseOrderLinePurchaseStock<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Stock moves", inverse = "purchase_line")]
    moves: Reference<BaseStockMove, MultipleIds>,
}
