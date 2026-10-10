use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference};
use stock::models::BaseStockMove;

/// The moves delivering an order line.
#[derive(Model)]
#[erp(id = "sale_order_line")]
#[erp(derived_model = "sale::models")]
#[allow(dead_code)]
pub struct SaleOrderLineSaleStock<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Stock moves", inverse = "sale_line")]
    moves: Reference<BaseStockMove, MultipleIds>,
}
