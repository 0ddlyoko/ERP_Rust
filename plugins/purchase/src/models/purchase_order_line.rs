use crate::models::product::{ProductPurchase, billed_on_receipt};
use crate::models::purchase_order::{BasePurchaseOrder, PurchaseOrder, PurchaseState};
use account::models::{
    AccountFiscalPosition, AccountInvoiceLine, AccountMove, AccountTax, BaseAccountInvoiceLine,
    BaseAccountTax, MoveState, MoveType, ProductAccount, TaxDocument,
};
use account::tax_engine::{self, LineInput};
use base::models::Contact;
use code_gen::{Model, erp_methods, selection};
use currency::models::Currency;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId, TimeDelta};
use erp::types::model::MapOfFields;
use product::models::{BaseProduct, Product};
use uom::models::{BaseUom, Uom};

#[selection]
pub enum BillStatus {
    #[default]
    #[selection(label = "Nothing to bill")]
    No,
    #[selection(label = "Waiting bills")]
    ToBill,
    #[selection(label = "Fully billed")]
    Billed,
}

/// A line of a request or purchase order: a product, how many, at the vendor's price.
#[derive(Model)]
#[erp(
    id = "purchase_order_line",
    order = "sequence, id",
    name_field = "name",
    methods
)]
#[allow(dead_code)]
pub struct PurchaseOrderLine<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    order: Reference<BasePurchaseOrder, SingleId>,
    #[erp(default = 10)]
    sequence: i32,
    #[erp(ondelete = "restrict", domain = r#"[["purchase_ok", "=", true]]"#)]
    product: Reference<BaseProduct, SingleId>,
    #[erp(label = "Description", compute = "compute_name", depends = ["product", "order.partner"], stored, editable)]
    name: Option<String>,
    #[erp(label = "Quantity", default = 1.0)]
    product_qty: Decimal,
    #[erp(label = "Unit", ondelete = "restrict", compute = "compute_uom", depends = ["product"], stored, editable)]
    uom: Reference<BaseUom, SingleId>,
    #[erp(
        label = "Unit price",
        compute = "compute_price_unit",
        depends = ["product", "product_qty", "order.partner", "order.currency"],
        stored,
        editable
    )]
    price_unit: Decimal,
    #[erp(label = "Discount (%)", default = 0.0)]
    discount: Decimal,
    #[erp(
        compute = "compute_taxes",
        depends = ["product"],
        stored,
        editable,
        relation = "purchase_order_line_tax_rel"
    )]
    taxes: Reference<BaseAccountTax, MultipleIds>,
    #[erp(label = "Expected arrival", compute = "compute_date_planned", depends = ["product", "order.partner", "order.date_order"], stored, editable)]
    date_planned: Option<NaiveDate>,
    #[erp(label = "Untaxed", compute = "compute_amounts", depends = ["product_qty", "price_unit", "discount", "taxes", "order.currency"], stored)]
    price_subtotal: Decimal,
    #[erp(label = "Tax", compute = "compute_amounts", depends = ["product_qty", "price_unit", "discount", "taxes", "order.currency"], stored)]
    price_tax: Decimal,
    #[erp(label = "Total", compute = "compute_amounts", depends = ["product_qty", "price_unit", "discount", "taxes", "order.currency"], stored)]
    price_total: Decimal,
    #[erp(label = "Received", default = 0.0)]
    qty_received: Decimal,
    #[erp(label = "Bill lines", relation = "purchase_order_line_invoice_rel")]
    bill_lines: Reference<BaseAccountInvoiceLine, MultipleIds>,
    #[erp(label = "Billed", compute = "compute_billed", depends = ["bill_lines.quantity", "bill_lines.move_id.state", "product_qty", "qty_received", "order.state"], stored)]
    qty_billed: Decimal,
    #[erp(label = "To bill", compute = "compute_billed", depends = ["bill_lines.quantity", "bill_lines.move_id.state", "product_qty", "qty_received", "order.state"], stored)]
    qty_to_bill: Decimal,
    #[erp(label = "Billing status", compute = "compute_billed", depends = ["bill_lines.quantity", "bill_lines.move_id.state", "product_qty", "qty_received", "order.state"], stored)]
    bill_status: BillStatus,
}

#[erp_methods]
impl PurchaseOrderLine<SingleId> {
    /// The vendor's price rule for the line's product and quantity, if it has one.
    fn seller(
        &self,
        env: &mut Environment,
    ) -> Result<Option<crate::models::ProductSupplierinfo<SingleId>>> {
        let product: Product<SingleId> = self.get_product(env)?;
        let order: PurchaseOrder<SingleId> = self.get_order(env)?;
        if product.is_empty() || order.is_empty() {
            return Ok(None);
        }
        let vendor: Contact<SingleId> = order.get_partner(env)?;
        if vendor.is_empty() {
            return Ok(None);
        }
        let quantity = *self.get_product_qty(env)?;
        let product: ProductPurchase<SingleId> = product.as_model();
        product.vendor_price(env, vendor.get_id(), quantity)
    }
}

#[erp_methods]
impl PurchaseOrderLine<MultipleIds> {
    /// The vendor's code and the product's name, and its purchase description below them.
    pub fn compute_name(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product: Product<SingleId> = line.get_product(env)?;
            let label = if product.is_empty() {
                None
            } else {
                let code = match line.seller(env)? {
                    Some(seller) => seller.get_product_code(&mut env.sudo())?.cloned(),
                    None => None,
                };
                let env = &mut *env.sudo();
                let name = match code {
                    Some(code) => format!("[{code}] {}", product.get_name(env)?),
                    None => product.get_display_name(env)?.clone(),
                };
                Some(match product.get_description_purchase(env)? {
                    Some(description) if !description.trim().is_empty() => {
                        format!("{name}\n{description}")
                    }
                    _ => name,
                })
            };
            line.set_name(label, env)?;
        }
        Ok(())
    }

    /// The unit the product is bought in.
    pub fn compute_uom(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product: Product<SingleId> = line.get_product(env)?;
            let uom: Uom<SingleId> = if product.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                product.get_purchase_uom(&mut env.sudo())?
            };
            line.set_uom(&uom, env)?;
        }
        Ok(())
    }

    /// The vendor's price for the quantity, else the product's cost converted to the purchase
    /// unit; in the order's currency.
    pub fn compute_price_unit(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product: Product<SingleId> = line.get_product(env)?;
            if product.is_empty() {
                line.set_price_unit(Decimal::ZERO, env)?;
                continue;
            }
            let price = match line.seller(env)? {
                Some(seller) => *seller.get_price(&mut env.sudo())?,
                None => {
                    let env = &mut *env.sudo();
                    let cost = *product.get_standard_price(env)?;
                    let unit: Uom<SingleId> = product.get_uom(env)?;
                    let purchase: Uom<SingleId> = product.get_purchase_uom(env)?;
                    if unit.is_empty() || purchase.is_empty() {
                        cost
                    } else {
                        unit.convert_price(env, cost, purchase)?
                    }
                }
            };
            let order: PurchaseOrder<SingleId> = line.get_order(env)?;
            let currency = order.currency_or_company(env)?;
            let company = Currency::of_company(env)?;
            let price = if currency.get_id() == company.get_id() {
                price
            } else {
                let date = *order.get_date_order(env)?;
                company.convert(env, price, currency, date)?
            };
            line.set_price_unit(price, env)?;
        }
        Ok(())
    }

    /// The order's date plus the vendor's lead time.
    pub fn compute_date_planned(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let order: PurchaseOrder<SingleId> = line.get_order(env)?;
            if order.is_empty() {
                line.set_date_planned(None::<NaiveDate>, env)?;
                continue;
            }
            let date = *order.get_date_order(env)?;
            let delay = match line.seller(env)? {
                Some(seller) => *seller.get_delay(&mut env.sudo())?,
                None => 0,
            };
            line.set_date_planned(Some(date + TimeDelta::days(delay as i64)), env)?;
        }
        Ok(())
    }

    /// The product's vendor taxes as the order's fiscal position maps them; none without a
    /// product. Taxes given by hand stay until the product changes.
    pub fn compute_taxes(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product: Product<SingleId> = line.get_product(env)?;
            if product.is_empty() {
                line.set_taxes(&AccountTax::<MultipleIds>::empty(env), env)?;
                continue;
            }
            let product: ProductAccount<SingleId> = product.as_model();
            let taxes: AccountTax<MultipleIds> = product.get_supplier_taxes(&mut env.sudo())?;
            let order: PurchaseOrder<SingleId> = line.get_order(env)?;
            let position: AccountFiscalPosition<SingleId> = order.get_fiscal_position(env)?;
            let mapped = position.map_taxes(env, taxes.get_ids_ref().clone())?;
            let mapped: AccountTax<MultipleIds> = AccountTax::from_ids(mapped, env);
            line.set_taxes(&mapped, env)?;
        }
        Ok(())
    }

    pub fn compute_amounts(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let order: PurchaseOrder<SingleId> = line.get_order(env)?;
            if order.is_empty() {
                line.set_price_subtotal(Decimal::ZERO, env)?;
                line.set_price_tax(Decimal::ZERO, env)?;
                line.set_price_total(Decimal::ZERO, env)?;
                continue;
            }
            let currency = order.currency_or_company(env)?;
            let rounding = *currency.get_rounding(&mut env.sudo())?;
            let taxes: AccountTax<MultipleIds> = line.get_taxes(env)?;
            let mut specs = Vec::new();
            for tax in &taxes {
                specs.push(tax.spec(env, TaxDocument::Invoice)?);
            }
            let result = tax_engine::compute(&LineInput {
                price_unit: *line.get_price_unit(env)?,
                quantity: *line.get_product_qty(env)?,
                discount: *line.get_discount(env)?,
                taxes: specs,
                rounding,
            })?;
            line.set_price_subtotal(result.subtotal, env)?;
            line.set_price_tax(result.total - result.subtotal, env)?;
            line.set_price_total(result.total, env)?;
        }
        Ok(())
    }

    /// What was billed — bills less vendor credit notes, cancelled ones left out — what is left
    /// to bill by the product's control, ordered or received, and so the line's status.
    pub fn compute_billed(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let mut billed = Decimal::ZERO;
            {
                let env = &mut *env.sudo();
                let bill_lines: AccountInvoiceLine<MultipleIds> = line.get_bill_lines(env)?;
                for bill_line in &bill_lines {
                    let bill: AccountMove<SingleId> = bill_line.get_move_id(env)?;
                    if bill.is_empty() || matches!(*bill.get_state(env)?, MoveState::Cancel) {
                        continue;
                    }
                    let quantity = *bill_line.get_quantity(env)?;
                    billed += match *bill.get_move_type(env)? {
                        MoveType::InRefund => -quantity,
                        _ => quantity,
                    };
                }
            }
            let order: PurchaseOrder<SingleId> = line.get_order(env)?;
            let confirmed = !order.is_empty() && order.is_state(env, PurchaseState::Purchase)?;
            let ordered = *line.get_product_qty(env)?;
            let product: Product<SingleId> = line.get_product(env)?;
            let due = if billed_on_receipt(env, &product)? {
                *line.get_qty_received(env)?
            } else {
                ordered
            };
            let to_bill = if confirmed {
                due - billed
            } else {
                Decimal::ZERO
            };
            let status = if !to_bill.is_zero() {
                BillStatus::ToBill
            } else if confirmed && billed >= ordered && !ordered.is_zero() {
                BillStatus::Billed
            } else {
                BillStatus::No
            };
            line.set_qty_billed(billed, env)?;
            line.set_qty_to_bill(to_bill, env)?;
            line.set_bill_status(status, env)?;
        }
        Ok(())
    }

    /// A quantity is not negative.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        for line in &values {
            if line
                .get_option::<&Decimal>("product_qty")
                .is_some_and(|quantity| *quantity < Decimal::ZERO)
            {
                return Err("An order line's quantity is not negative".into());
            }
        }
        sup.call_with(values, env)
    }

    /// Lines of a cancelled order stay as they are; those of a confirmed order keep their
    /// product.
    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        if values
            .get_option::<&Decimal>("product_qty")
            .is_some_and(|quantity| *quantity < Decimal::ZERO)
        {
            return Err("An order line's quantity is not negative".into());
        }
        const PRICED: [&str; 6] = [
            "product",
            "product_qty",
            "price_unit",
            "discount",
            "taxes",
            "uom",
        ];
        if values
            .fields
            .keys()
            .any(|field| PRICED.contains(&field.as_str()))
        {
            for line in self {
                let order: PurchaseOrder<SingleId> = line.get_order(env)?;
                if order.is_state(env, PurchaseState::Cancel)? {
                    return Err(format!(
                        "{} is cancelled: its lines cannot change",
                        order.get_name(env)?
                    )
                    .into());
                }
                if values.contains_key("product") && order.is_state(env, PurchaseState::Purchase)? {
                    return Err(format!(
                        "{} is confirmed: add a line rather than change its product",
                        order.get_name(env)?
                    )
                    .into());
                }
            }
        }
        sup.call_with(values, env)
    }

    /// Lines billed already stay on their order.
    pub fn delete(&self, env: &mut Environment, sup: Super) -> Result<u32> {
        for line in self {
            if !line.get_qty_billed(env)?.is_zero() {
                return Err(format!(
                    "\"{}\" is billed: it stays on its order",
                    line.get_name(env)?.cloned().unwrap_or_default()
                )
                .into());
            }
        }
        sup.call(env)
    }
}
