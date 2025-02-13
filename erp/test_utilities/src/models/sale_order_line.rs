use std::error::Error;
use code_gen::Model;
use erp::environment::Environment;
use erp::field::{IdMode, MultipleIds, Reference, SingleId};
use crate::models::sale_order::BaseSaleOrder;

#[derive(Model, Debug)]
#[erp(table_name="sale_order_line")]
#[allow(dead_code)]
pub struct SaleOrderLine<Mode: IdMode> {
    pub id: Mode,
    order: Reference<BaseSaleOrder, SingleId>,
    #[erp(default=42)]
    price: i32,
    #[erp(default=10)]
    amount: i32,
    #[erp(compute="compute_total_price", depends=["price", "amount"])]
    total_price: i32,
}

impl SaleOrderLine<MultipleIds> {
    pub fn compute_total_price(
        &self,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error>> {
        for sale_order_line in self {
            let price = *sale_order_line.get_price(env)?;
            let amount = *sale_order_line.get_amount(env)?;
            sale_order_line.set_total_price(price * amount, env)?;
        }

        Ok(())
    }
}
