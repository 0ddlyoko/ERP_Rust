use crate::models::product_supplierinfo::{BaseProductSupplierinfo, SupplierInfo};
use crate::models::purchase_order::{BasePurchaseOrder, PurchaseOrder};
use crate::models::purchase_order_line::{BasePurchaseOrderLine, PurchaseOrderLine};
use crate::vendor_price::{self, VendorPrice};
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use product::models::Product;
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
    #[erp(
        label = "Purchased lines",
        inverse = "product",
        domain = r#"[["order.state", "=", "purchase"]]"#
    )]
    purchased_lines: Reference<BasePurchaseOrderLine, MultipleIds>,
    #[erp(
        label = "Purchase orders",
        compute = "compute_purchases",
        depends = ["purchased_lines.order.state", "purchased_lines.product_qty", "purchased_lines.uom", "uom"]
    )]
    purchase_orders: Reference<BasePurchaseOrder, MultipleIds>,
    #[erp(
        label = "Purchased",
        compute = "compute_purchases",
        depends = ["purchased_lines.order.state", "purchased_lines.product_qty", "purchased_lines.uom", "uom"]
    )]
    purchased_qty: Decimal,
}

#[erp_methods]
impl ProductPurchase<MultipleIds> {
    /// The confirmed purchase orders holding the product, and how much of it they bought, in its
    /// unit.
    pub fn compute_purchases(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        for product in self {
            let unit: Uom<SingleId> = product.get_uom(env)?;
            let confirmed: PurchaseOrderLine<MultipleIds> = product.get_purchased_lines(env)?;
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
