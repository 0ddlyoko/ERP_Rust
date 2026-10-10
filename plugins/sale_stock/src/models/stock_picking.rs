use crate::models::stock_move::StockMoveSaleStock;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, SingleId};
use erp_search_code_gen::make_domain;
use sale::models::SaleOrderLine;
use stock::models::{MoveStatus, StockLocation, StockMove, StockPicking};

/// Deliveries done count on their orders' lines.
#[derive(Model)]
#[erp(id = "stock_picking", methods)]
#[erp(derived_model = "stock::models")]
#[allow(dead_code)]
pub struct StockPickingSaleStock<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl StockPickingSaleStock<MultipleIds> {
    /// Each order line delivered by these transfers counts what its moves took to customers,
    /// less what came back.
    pub fn on_done(&self, env: &mut Environment, sup: Super) -> Result<()> {
        sup.call(env)?;
        let env = &mut *env.sudo();
        let pickings: StockPicking<MultipleIds> = self.as_model();
        let moves: StockMove<MultipleIds> = pickings.get_moves(env)?;
        let moves: StockMoveSaleStock<MultipleIds> = moves.as_model();
        let lines: SaleOrderLine<MultipleIds> = moves.get_sale_line(env)?;
        for line in &lines {
            let delivered = delivered_quantity(env, line.get_id())?;
            line.set_qty_delivered(delivered, env)?;
        }
        Ok(())
    }
}

/// What the done moves of an order line took to customers, less what they brought back, in the
/// line's unit.
fn delivered_quantity(env: &mut Environment, line: u32) -> Result<Decimal> {
    let moves: StockMove<MultipleIds> = env.search(&make_domain!([
        ("sale_line", "=", line),
        ("state", "=", MoveStatus::Done)
    ]))?;
    let mut delivered = Decimal::ZERO;
    for stock_move in &moves {
        let source: StockLocation<SingleId> = stock_move.get_location(env)?;
        let destination: StockLocation<SingleId> = stock_move.get_location_dest(env)?;
        let quantity = *stock_move.get_quantity(env)?;
        if source.is_internal(env)? && !destination.is_internal(env)? {
            delivered += quantity;
        } else if !source.is_internal(env)? && destination.is_internal(env)? {
            delivered -= quantity;
        }
    }
    Ok(delivered)
}
