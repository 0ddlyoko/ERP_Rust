use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use sale::models::{BaseSaleOrderLine, SaleOrderLine};

/// The order line a move delivers.
#[derive(Model)]
#[erp(id = "stock_move", methods)]
#[erp(derived_model = "stock::models")]
#[allow(dead_code)]
pub struct StockMoveSaleStock<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Sales order line", ondelete = "set_null")]
    sale_line: Reference<BaseSaleOrderLine, SingleId>,
}

#[erp_methods]
impl StockMoveSaleStock<MultipleIds> {
    /// A back order or a return still delivers the same order line.
    pub fn copy_values(&self, env: &mut Environment, sup: Super) -> Result<MapOfFields> {
        let mut values: MapOfFields = sup.call(env)?;
        if let Some(stock_move) = self.into_iter().next() {
            let line: SaleOrderLine<SingleId> = stock_move.get_sale_line(&mut env.sudo())?;
            if let Some(line) = line.get_optional_id() {
                values.insert("sale_line", line);
            }
        }
        Ok(values)
    }
}
