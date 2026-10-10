use crate::models::stock_move::StockMovePurchase;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, SingleId};
use erp_search_code_gen::make_domain;
use purchase::models::PurchaseOrderLine;
use stock::models::{Location, MoveStatus, Picking, StockMove};

/// Receipts done count on their orders' lines.
#[derive(Model)]
#[erp(id = "stock_picking", methods)]
#[erp(derived_model = "stock::models")]
#[allow(dead_code)]
pub struct PickingPurchase<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl PickingPurchase<MultipleIds> {
    /// Each order line received by these transfers counts what its moves brought into stock,
    /// less what went back.
    pub fn on_done(&self, env: &mut Environment, sup: Super) -> Result<()> {
        sup.call(env)?;
        let env = &mut *env.sudo();
        let pickings: Picking<MultipleIds> = self.as_model();
        let moves: StockMove<MultipleIds> = pickings.get_moves(env)?;
        let moves: StockMovePurchase<MultipleIds> = moves.as_model();
        let lines: PurchaseOrderLine<MultipleIds> = moves.get_purchase_line(env)?;
        for line in &lines {
            let received = received_quantity(env, line.get_id())?;
            line.set_qty_received(received, env)?;
        }
        Ok(())
    }
}

/// What the done moves of an order line brought into stock, less what they sent back, in the
/// line's unit.
fn received_quantity(env: &mut Environment, line: u32) -> Result<Decimal> {
    let moves: StockMove<MultipleIds> = env.search(&make_domain!([
        ("purchase_line", "=", line),
        ("state", "=", MoveStatus::Done)
    ]))?;
    let mut received = Decimal::ZERO;
    for stock_move in &moves {
        let source: Location<SingleId> = stock_move.get_location(env)?;
        let destination: Location<SingleId> = stock_move.get_location_dest(env)?;
        let quantity = *stock_move.get_quantity(env)?;
        if !source.is_internal(env)? && destination.is_internal(env)? {
            received += quantity;
        } else if source.is_internal(env)? && !destination.is_internal(env)? {
            received -= quantity;
        }
    }
    Ok(received)
}
