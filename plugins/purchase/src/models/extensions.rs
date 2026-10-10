use crate::models::purchase_order::{BasePurchaseOrder, PurchaseOrder, PurchaseState};
use crate::models::purchase_order_line::{BasePurchaseOrderLine, PurchaseOrderLine};
use crate::vendor_price::{self, VendorPrice};
use account::models::{InvoiceLine, Move};
use base::models::BaseContact;
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use product::models::{BaseProduct, Product};
use uom::conversion::Rounding;
use uom::models::{BaseUom, Uom};

#[selection]
pub enum BillPolicy {
    #[default]
    #[selection(label = "Ordered quantities")]
    Order,
    #[selection(label = "Received quantities")]
    Receive,
}

/// The price a vendor asks for a product, from a quantity on, and how long it takes to come.
#[derive(Model)]
#[erp(id = "product_supplierinfo", order = "sequence, id")]
#[allow(dead_code)]
pub struct SupplierInfo<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Vendor", required, ondelete = "cascade")]
    partner: Reference<BaseContact, SingleId>,
    #[erp(required, ondelete = "cascade")]
    product: Reference<BaseProduct, SingleId>,
    #[erp(label = "Vendor product code")]
    product_code: Option<String>,
    #[erp(label = "Minimum quantity", default = 0.0)]
    min_quantity: Decimal,
    #[erp(default = 0.0)]
    price: Decimal,
    #[erp(label = "Delivery lead time (days)", default = 1)]
    delay: i32,
    #[erp(default = 10)]
    sequence: i32,
}

/// How a product is billed, and the vendors selling it.
#[derive(Model)]
#[erp(id = "product", methods)]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductPurchase<Mode: IdMode> {
    id: Mode,
    uom: Reference<BaseUom, SingleId>,
    #[erp(label = "Bill control")]
    purchase_method: BillPolicy,
    #[erp(label = "Vendors", inverse = "product")]
    sellers: Reference<BaseProductSupplierinfo, MultipleIds>,
    #[erp(label = "Purchase order lines", inverse = "product")]
    purchase_lines: Reference<BasePurchaseOrderLine, MultipleIds>,
    #[erp(
        label = "Purchase orders",
        compute = "compute_purchases",
        depends = ["purchase_lines.order.state", "purchase_lines.product_qty", "purchase_lines.uom", "uom"]
    )]
    purchase_orders: Reference<BasePurchaseOrder, MultipleIds>,
    #[erp(
        label = "Purchased",
        compute = "compute_purchases",
        depends = ["purchase_lines.order.state", "purchase_lines.product_qty", "purchase_lines.uom", "uom"]
    )]
    purchased_qty: Decimal,
}

#[erp_methods]
impl ProductPurchase<MultipleIds> {
    /// The confirmed purchase orders holding the product, and how much of it they bought, in its
    /// unit.
    pub fn compute_purchases(&self, env: &mut Environment) -> Result<()> {
        for product in self {
            let env = &mut *env.sudo();
            let unit: Uom<SingleId> = product.get_uom(env)?;
            let lines: PurchaseOrderLine<MultipleIds> = product.get_purchase_lines(env)?;
            let confirmed = lines.filtered(env, |line, env| {
                let order: PurchaseOrder<SingleId> = line.get_order(env)?;
                order.is_state(env, PurchaseState::Purchase)
            })?;
            let bought: Decimal = confirmed.sum(env, |line, env| {
                let line_unit: Uom<SingleId> = line.get_uom(env)?;
                let quantity = *line.get_product_qty(env)?;
                line_unit.convert_to(env, quantity, unit.clone(), Rounding::HalfUp)
            })?;
            let orders: PurchaseOrder<MultipleIds> = confirmed.get_order(env)?;
            product.set_purchase_orders(&orders, env)?;
            product.set_purchased_qty(bought, env)?;
        }
        Ok(())
    }
}

#[erp_methods]
impl ProductPurchase<SingleId> {
    /// The price `vendor` asks for `quantity` of the product, in the purchase unit; none when it
    /// asks none.
    pub fn vendor_price(
        &self,
        env: &mut Environment,
        vendor: u32,
        quantity: Decimal,
    ) -> Result<Option<SupplierInfo<SingleId>>> {
        let env = &mut *env.sudo();
        let sellers: SupplierInfo<MultipleIds> = self.get_sellers(env)?;
        let mut prices = Vec::new();
        for seller in &sellers {
            let partner: base::models::Contact<SingleId> = seller.get_partner(env)?;
            prices.push(VendorPrice {
                id: seller.get_id(),
                vendor: partner.get_id(),
                min_quantity: *seller.get_min_quantity(env)?,
                price: *seller.get_price(env)?,
                sequence: *seller.get_sequence(env)?,
            });
        }
        Ok(vendor_price::best_price(&prices, vendor, quantity)
            .map(|price| env.get_record(price.id.into())))
    }
}

/// The purchase order lines a vendor bill line bills.
#[derive(Model)]
#[erp(id = "account_invoice_line")]
#[erp(derived_model = "account::models")]
#[allow(dead_code)]
pub struct InvoiceLinePurchase<Mode: IdMode> {
    id: Mode,
    #[erp(
        label = "Purchase order lines",
        relation = "purchase_order_line_invoice_rel"
    )]
    purchase_lines: Reference<BasePurchaseOrderLine, MultipleIds>,
}

/// A vendor credit note of an order's bill counts against the order.
#[derive(Model)]
#[erp(id = "account_move", methods)]
#[erp(derived_model = "account::models")]
#[allow(dead_code)]
pub struct MovePurchase<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl MovePurchase<MultipleIds> {
    /// Each line of the credit note bills back the order lines its bill's line billed.
    pub fn link_reversal(&self, env: &mut Environment, reversal: u32, sup: Super) -> Result<()> {
        sup.call(env)?;
        let env = &mut *env.sudo();
        let reversal: Move<SingleId> = env.get_record(reversal.into());
        let copies: InvoiceLine<MultipleIds> = reversal.get_invoice_lines(env)?;
        for origin in self {
            let origin: Move<SingleId> = origin.as_model();
            let originals: InvoiceLine<MultipleIds> = origin.get_invoice_lines(env)?;
            for (original, copy) in originals.into_iter().zip(copies.clone()) {
                let original: InvoiceLinePurchase<SingleId> = original.as_model();
                let order_lines: PurchaseOrderLine<MultipleIds> =
                    original.get_purchase_lines(env)?;
                if order_lines.is_empty() {
                    continue;
                }
                let copy: InvoiceLinePurchase<SingleId> = copy.as_model();
                copy.set_purchase_lines(&order_lines, env)?;
            }
        }
        Ok(())
    }
}

/// Whether the product is billed as received.
pub fn billed_on_receipt(env: &mut Environment, product: &Product<SingleId>) -> Result<bool> {
    if product.is_empty() {
        return Ok(false);
    }
    let env = &mut *env.sudo();
    let product: ProductPurchase<SingleId> = product.as_model();
    Ok(matches!(
        *product.get_purchase_method(env)?,
        BillPolicy::Receive
    ))
}
