use std::error::Error;
use code_gen::Model;
use erp::environment::Environment;
use erp::field::{IdMode, Reference, SingleId};
use crate::models::sale_order::BaseSaleOrder;

#[derive(Model, Debug)]
#[erp(table_name="sale_order_line")]
pub struct SaleOrderLine<Mode: IdMode> {
    pub id: Mode,
    order: Reference<BaseSaleOrder, SingleId>,
    price: i32,
    amount: i32,
    #[erp(compute="compute_total_price", depends=["price", "amount"])]
    total_price: i32,
}

impl<Mode: IdMode> SaleOrderLine<Mode> {
    pub fn compute_total_price(
        &mut self,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error>> {
        // self.set_total_price(*self.get_price(env)? * *self.get_amount(env)?, env)

        Ok(())
    }
}
