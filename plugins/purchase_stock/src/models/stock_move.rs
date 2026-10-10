use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use purchase::models::{BasePurchaseOrderLine, PurchaseOrderLine};

/// The order line a move receives.
#[derive(Model)]
#[erp(id = "stock_move", methods)]
#[erp(derived_model = "stock::models")]
#[allow(dead_code)]
pub struct StockMovePurchase<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Purchase order line", ondelete = "set_null")]
    purchase_line: Reference<BasePurchaseOrderLine, SingleId>,
}

#[erp_methods]
impl StockMovePurchase<MultipleIds> {
    /// A back order or a return still receives the same order line.
    pub fn copy_values(&self, env: &mut Environment, sup: Super) -> Result<MapOfFields> {
        let mut values: MapOfFields = sup.call(env)?;
        if let Some(stock_move) = self.into_iter().next() {
            let line: PurchaseOrderLine<SingleId> =
                stock_move.get_purchase_line(&mut env.sudo())?;
            if let Some(line) = line.get_optional_id() {
                values.insert("purchase_line", line);
            }
        }
        Ok(values)
    }
}
