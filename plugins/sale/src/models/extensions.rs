use crate::models::pricelist::BaseProductPricelist;
use crate::models::sale_order::{BaseSaleOrder, SaleOrder, SaleState};
use crate::models::sale_order_line::{BaseSaleOrderLine, SaleOrderLine};
use account::models::{InvoiceLine, Move};
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

/// The prices a customer is offered.
#[derive(Model)]
#[erp(id = "contact")]
#[erp(derived_model = "base::models")]
#[allow(dead_code)]
pub struct ContactSale<Mode: IdMode> {
    id: Mode,
    #[erp(ondelete = "set_null")]
    pricelist: Reference<BaseProductPricelist, SingleId>,
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
    #[erp(label = "Sales order lines", inverse = "product")]
    sale_lines: Reference<BaseSaleOrderLine, MultipleIds>,
    #[erp(
        label = "Sales orders",
        compute = "compute_sales",
        depends = ["sale_lines.order.state", "sale_lines.product_uom_qty", "sale_lines.uom", "uom"]
    )]
    sale_orders: Reference<BaseSaleOrder, MultipleIds>,
    #[erp(
        label = "Sold",
        compute = "compute_sales",
        depends = ["sale_lines.order.state", "sale_lines.product_uom_qty", "sale_lines.uom", "uom"]
    )]
    sold_qty: Decimal,
}

#[erp_methods]
impl ProductSale<MultipleIds> {
    /// The confirmed sales orders holding the product, and how much of it they sold, in its unit.
    pub fn compute_sales(&self, env: &mut Environment) -> Result<()> {
        for product in self {
            let env = &mut *env.sudo();
            let unit: Uom<SingleId> = product.get_uom(env)?;
            let lines: SaleOrderLine<MultipleIds> = product.get_sale_lines(env)?;
            let confirmed = lines.filtered(env, |line, env| {
                let order: SaleOrder<SingleId> = line.get_order(env)?;
                order.is_state(env, SaleState::Sale)
            })?;
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

/// The order lines an invoice line invoices.
#[derive(Model)]
#[erp(id = "account_invoice_line")]
#[erp(derived_model = "account::models")]
#[allow(dead_code)]
pub struct InvoiceLineSale<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Order lines", relation = "sale_order_line_invoice_rel")]
    sale_lines: Reference<BaseSaleOrderLine, MultipleIds>,
}

/// A credit note of an order's invoice counts against the order.
#[derive(Model)]
#[erp(id = "account_move", methods)]
#[erp(derived_model = "account::models")]
#[allow(dead_code)]
pub struct MoveSale<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl MoveSale<MultipleIds> {
    /// Each line of the credit note invoices back the order lines its invoice's line invoiced:
    /// the lines were copied in order.
    pub fn link_reversal(&self, env: &mut Environment, reversal: u32, sup: Super) -> Result<()> {
        sup.call(env)?;
        let env = &mut *env.sudo();
        let reversal: Move<SingleId> = env.get_record(reversal.into());
        let copies: InvoiceLine<MultipleIds> = reversal.get_invoice_lines(env)?;
        for origin in self {
            let origin: Move<SingleId> = origin.as_model();
            let originals: InvoiceLine<MultipleIds> = origin.get_invoice_lines(env)?;
            for (original, copy) in originals.into_iter().zip(copies.clone()) {
                let original: InvoiceLineSale<SingleId> = original.as_model();
                let order_lines: SaleOrderLine<MultipleIds> = original.get_sale_lines(env)?;
                if order_lines.is_empty() {
                    continue;
                }
                let copy: InvoiceLineSale<SingleId> = copy.as_model();
                copy.set_sale_lines(&order_lines, env)?;
            }
        }
        Ok(())
    }
}
