use crate::models::{BaseSaleOrderLine, BaseTag, SaleOrderLine};
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference};

#[selection]
pub enum SaleOrderState {
    #[default]
    Draft,
    Sent,
    Paid,
    Cancelled,
}

#[derive(Model)]
#[erp(id = "sale_order", methods)]
#[allow(dead_code)]
pub struct SaleOrder<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "0ddlyoko", index = "trigram")]
    name: String,
    #[erp(index)]
    state: SaleOrderState,
    #[erp(compute = "compute_total_price", depends = ["lines.total_price"], stored)]
    total_price: i32,
    #[erp(inverse = "order")]
    lines: Reference<BaseSaleOrderLine, MultipleIds>,
    #[erp(relation = "sale_order_tag_rel")]
    tags: Reference<BaseTag, MultipleIds>,
}

#[erp_methods]
impl SaleOrder<MultipleIds> {
    pub fn compute_total_price(&self, env: &mut Environment) -> Result<()> {
        for sale_order in self {
            let lines: SaleOrderLine<_> = sale_order.get_lines(env)?;
            let total_prices = lines.get_total_price(env)?;
            sale_order.set_total_price(total_prices.into_iter().sum(), env)?;
        }
        Ok(())
    }
}
