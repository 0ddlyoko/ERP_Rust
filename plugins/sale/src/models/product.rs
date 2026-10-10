use crate::models::sale_order::{BaseSaleOrder, SaleOrder};
use crate::models::sale_order_line::{BaseSaleOrderLine, SaleOrderLine};
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use uom::conversion::Rounding;
use uom::models::{BaseUom, Uom};

#[selection]
pub enum InvoicePolicy {
    #[default]
    #[selection(label = "Ordered quantities")]
    Order,
    #[selection(label = "Delivered quantities")]
    Delivery,
}

/// Whether a product is invoiced as ordered or as delivered, and how much of it was sold.
#[derive(Model)]
#[erp(id = "product", methods)]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductSale<Mode: IdMode> {
    id: Mode,
    uom: Reference<BaseUom, SingleId>,
    #[erp(label = "Invoicing policy")]
    invoice_policy: InvoicePolicy,
    #[erp(
        label = "Sold lines",
        inverse = "product",
        domain = r#"[["order.state", "=", "sale"]]"#
    )]
    sold_lines: Reference<BaseSaleOrderLine, MultipleIds>,
    #[erp(
        label = "Sales orders",
        compute = "compute_sales",
        depends = ["sold_lines.order.state", "sold_lines.product_uom_qty", "sold_lines.uom", "uom"]
    )]
    sale_orders: Reference<BaseSaleOrder, MultipleIds>,
    #[erp(
        label = "Sold",
        compute = "compute_sales",
        depends = ["sold_lines.order.state", "sold_lines.product_uom_qty", "sold_lines.uom", "uom"]
    )]
    sold_qty: Decimal,
}

#[erp_methods]
impl ProductSale<MultipleIds> {
    /// The confirmed sales orders holding the product, and how much of it they sold, in its unit.
    pub fn compute_sales(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        for product in self {
            let unit: Uom<SingleId> = product.get_uom(env)?;
            let confirmed: SaleOrderLine<MultipleIds> = product.get_sold_lines(env)?;
            let sold: Decimal = confirmed.sum(env, |line, env| {
                let line_unit: Uom<SingleId> = line.get_uom(env)?;
                let quantity = *line.get_product_uom_qty(env)?;
                line_unit.convert_to(env, quantity, unit.clone(), Rounding::HalfUp)
            })?;
            let orders: SaleOrder<MultipleIds> = confirmed.get_order(env)?;
            product.set_sale_orders(&orders, env)?;
            product.set_sold_qty(sold, env)?;
        }
        Ok(())
    }
}
