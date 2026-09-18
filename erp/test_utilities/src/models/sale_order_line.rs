use crate::models::sale_order::BaseSaleOrder;
use crate::models::{SaleOrder, Tag};
use code_gen::Model;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId, Super};
use std::error::Error;

#[derive(Model, Debug)]
#[erp(id = "sale_order_line")]
#[allow(dead_code)]
pub struct SaleOrderLine<Mode: IdMode> {
    pub id: Mode,
    order: Reference<BaseSaleOrder, SingleId>,
    #[erp(default = 42)]
    price: i32,
    #[erp(default = 10)]
    amount: i32,
    #[erp(compute="compute_total_price", depends=["price", "amount"])]
    total_price: i32,
    /// Three segments, crossing a many2one then a one2many.
    #[erp(compute = "compute_siblings_total", depends = ["order.lines.price"])]
    siblings_total: i32,
    /// Three segments, crossing a many2one then a many2many.
    #[erp(compute = "compute_order_tags", depends = ["order.tags.name"])]
    order_tags: String,
}

impl SaleOrderLine<MultipleIds> {
    pub fn compute_total_price(
        &self,
        env: &mut Environment,
        _parent: Super,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for sale_order_line in self {
            let price = *sale_order_line.get_price(env)?;
            let amount = *sale_order_line.get_amount(env)?;
            sale_order_line.set_total_price(price * amount, env)?;
        }

        Ok(())
    }

    pub fn compute_siblings_total(
        &self,
        env: &mut Environment,
        _parent: Super,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for line in self {
            let total = match line.get_order::<SaleOrder<SingleId>>(env)? {
                Some(order) => {
                    let siblings: SaleOrderLine<MultipleIds> = order.get_lines(env)?;
                    siblings.get_price(env)?.into_iter().sum()
                }
                None => 0,
            };
            line.set_siblings_total(total, env)?;
        }
        Ok(())
    }

    pub fn compute_order_tags(
        &self,
        env: &mut Environment,
        _parent: Super,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for line in self {
            let summary = match line.get_order::<SaleOrder<SingleId>>(env)? {
                Some(order) => {
                    let tags: Tag<MultipleIds> = order.get_tags(env)?;
                    let mut names: Vec<String> = tags.get_name(env)?.into_iter().cloned().collect();
                    names.sort();
                    names.join(",")
                }
                None => String::new(),
            };
            line.set_order_tags(summary, env)?;
        }
        Ok(())
    }
}
