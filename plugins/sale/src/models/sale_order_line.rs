use crate::models::extensions::{InvoicePolicy, ProductSale};
use crate::models::pricelist::Pricelist;
use crate::models::sale_order::{BaseSaleOrder, SaleOrder, SaleState};
use account::models::{
    BaseAccountInvoiceLine, BaseAccountTax, FiscalPosition, InvoiceLine, Move, MoveState, MoveType,
    ProductAccount, Tax, TaxDocument,
};
use account::tax_engine::{self, LineInput};
use code_gen::{Model, erp_methods, selection};
use currency::models::Currency;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use product::models::{BaseProduct, Product};
use uom::models::{BaseUom, Uom};

#[selection]
pub enum InvoiceStatus {
    #[default]
    #[selection(label = "Nothing to invoice")]
    No,
    #[selection(label = "To invoice")]
    ToInvoice,
    #[selection(label = "Fully invoiced")]
    Invoiced,
}

/// A line of a quotation or order: a product, how many, at the pricelist's price.
#[derive(Model)]
#[erp(
    id = "sale_order_line",
    order = "sequence, id",
    name_field = "name",
    methods
)]
#[allow(dead_code)]
pub struct SaleOrderLine<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    order: Reference<BaseSaleOrder, SingleId>,
    #[erp(default = 10)]
    sequence: i32,
    #[erp(ondelete = "restrict", domain = r#"[["sale_ok", "=", true]]"#)]
    product: Reference<BaseProduct, SingleId>,
    #[erp(label = "Description", compute = "compute_name", depends = ["product"], stored, editable)]
    name: Option<String>,
    #[erp(label = "Quantity", default = 1.0)]
    product_uom_qty: Decimal,
    #[erp(
        label = "Unit",
        ondelete = "restrict",
        compute = "compute_uom",
        depends = ["product"],
        stored,
        editable
    )]
    uom: Reference<BaseUom, SingleId>,
    #[erp(
        label = "Unit price",
        compute = "compute_price_unit",
        depends = ["product", "product_uom_qty", "order.pricelist"],
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
        relation = "sale_order_line_tax_rel"
    )]
    taxes: Reference<BaseAccountTax, MultipleIds>,
    #[erp(
        label = "Untaxed",
        compute = "compute_amounts",
        depends = ["product_uom_qty", "price_unit", "discount", "taxes", "order.currency"],
        stored
    )]
    price_subtotal: Decimal,
    #[erp(
        label = "Tax",
        compute = "compute_amounts",
        depends = ["product_uom_qty", "price_unit", "discount", "taxes", "order.currency"],
        stored
    )]
    price_tax: Decimal,
    #[erp(
        label = "Total",
        compute = "compute_amounts",
        depends = ["product_uom_qty", "price_unit", "discount", "taxes", "order.currency"],
        stored
    )]
    price_total: Decimal,
    #[erp(label = "Delivered", default = 0.0)]
    qty_delivered: Decimal,
    #[erp(label = "Invoice lines", relation = "sale_order_line_invoice_rel")]
    invoice_lines: Reference<BaseAccountInvoiceLine, MultipleIds>,
    #[erp(
        label = "Invoiced",
        compute = "compute_invoiced",
        depends = ["invoice_lines.quantity", "invoice_lines.move_id.state", "product_uom_qty", "qty_delivered", "order.state"],
        stored
    )]
    qty_invoiced: Decimal,
    #[erp(
        label = "To invoice",
        compute = "compute_invoiced",
        depends = ["invoice_lines.quantity", "invoice_lines.move_id.state", "product_uom_qty", "qty_delivered", "order.state"],
        stored
    )]
    qty_to_invoice: Decimal,
    #[erp(
        label = "Invoice status",
        compute = "compute_invoiced",
        depends = ["invoice_lines.quantity", "invoice_lines.move_id.state", "product_uom_qty", "qty_delivered", "order.state"],
        stored
    )]
    invoice_status: InvoiceStatus,
}

#[erp_methods]
impl SaleOrderLine<SingleId> {
    /// What the line's product says, read as sudo: the person quoting may not manage products.
    fn product_record(&self, env: &mut Environment) -> Result<Product<SingleId>> {
        self.get_product(env)
    }
}

#[erp_methods]
impl SaleOrderLine<MultipleIds> {
    /// The product's name, and its sales description below it. Without a product, nothing.
    pub fn compute_name(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product = line.product_record(env)?;
            let label = if product.is_empty() {
                None
            } else {
                let env = &mut *env.sudo();
                let name = product.get_display_name(env)?.clone();
                Some(match product.get_description_sale(env)? {
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

    /// The unit the product is sold in.
    pub fn compute_uom(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product = line.product_record(env)?;
            let uom: Uom<SingleId> = if product.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                product.get_uom(&mut env.sudo())?
            };
            line.set_uom(&uom, env)?;
        }
        Ok(())
    }

    /// The pricelist's price for the quantity on the order's date, in the line's unit and the
    /// order's currency. Without a product, nothing.
    pub fn compute_price_unit(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product = line.product_record(env)?;
            if product.is_empty() {
                line.set_price_unit(Decimal::ZERO, env)?;
                continue;
            }
            let order: SaleOrder<SingleId> = line.get_order(env)?;
            let quantity = *line.get_product_uom_qty(env)?;
            let date = *order.get_date_order(env)?;
            let pricelist: Pricelist<SingleId> = order.get_pricelist(env)?;
            let mut price = pricelist.price_of(env, product.clone(), quantity, date)?;
            let uom: Uom<SingleId> = line.get_uom(env)?;
            if !uom.is_empty() {
                let env = &mut *env.sudo();
                let product_uom: Uom<SingleId> = product.get_uom(env)?;
                price = product_uom.convert_price(env, price, uom)?;
            }
            let currency = order.currency_or_company(env)?;
            let company = Currency::of_company(env)?;
            if currency.get_id() != company.get_id() {
                price = company.convert(env, price, currency, date)?;
            }
            line.set_price_unit(price, env)?;
        }
        Ok(())
    }

    /// The product's customer taxes as the order's fiscal position maps them; none without a
    /// product. Taxes given by hand stay until the product changes.
    pub fn compute_taxes(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product = line.product_record(env)?;
            if product.is_empty() {
                line.set_taxes(&Tax::<MultipleIds>::from_ids(Vec::<u32>::new(), env), env)?;
                continue;
            }
            let taxes: Tax<MultipleIds> = {
                let env = &mut *env.sudo();
                let product: ProductAccount<SingleId> = env.get_record(product.get_id().into());
                product.get_taxes(env)?
            };
            let order: SaleOrder<SingleId> = line.get_order(env)?;
            let position: FiscalPosition<SingleId> = order.get_fiscal_position(env)?;
            let mapped = position.map_taxes(env, taxes.get_ids_ref().clone())?;
            let mapped: Tax<MultipleIds> = Tax::from_ids(mapped, env);
            line.set_taxes(&mapped, env)?;
        }
        Ok(())
    }

    pub fn compute_amounts(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let order: SaleOrder<SingleId> = line.get_order(env)?;
            if order.is_empty() {
                line.set_price_subtotal(Decimal::ZERO, env)?;
                line.set_price_tax(Decimal::ZERO, env)?;
                line.set_price_total(Decimal::ZERO, env)?;
                continue;
            }
            let currency = order.currency_or_company(env)?;
            let rounding = *currency.get_rounding(&mut env.sudo())?;
            let taxes: Tax<MultipleIds> = line.get_taxes(env)?;
            let mut specs = Vec::new();
            for tax in &taxes {
                specs.push(tax.spec(env, TaxDocument::Invoice)?);
            }
            let result = tax_engine::compute(&LineInput {
                price_unit: *line.get_price_unit(env)?,
                quantity: *line.get_product_uom_qty(env)?,
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

    /// What was invoiced — invoices less credit notes, cancelled ones left out — what is left to
    /// invoice by the product's policy, ordered or delivered, and so the line's status.
    pub fn compute_invoiced(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let mut invoiced = Decimal::ZERO;
            {
                let env = &mut *env.sudo();
                let invoice_lines: InvoiceLine<MultipleIds> = line.get_invoice_lines(env)?;
                for invoice_line in &invoice_lines {
                    let invoice: Move<SingleId> = invoice_line.get_move_id(env)?;
                    if invoice.is_empty() || matches!(*invoice.get_state(env)?, MoveState::Cancel) {
                        continue;
                    }
                    let quantity = *invoice_line.get_quantity(env)?;
                    invoiced += match *invoice.get_move_type(env)? {
                        MoveType::OutRefund => -quantity,
                        _ => quantity,
                    };
                }
            }
            let order: SaleOrder<SingleId> = line.get_order(env)?;
            let confirmed = !order.is_empty() && order.is_state(env, SaleState::Sale)?;
            let ordered = *line.get_product_uom_qty(env)?;
            let product = line.product_record(env)?;
            let by_delivery = !product.is_empty() && {
                let env = &mut *env.sudo();
                let product: ProductSale<SingleId> = env.get_record(product.get_id().into());
                matches!(*product.get_invoice_policy(env)?, InvoicePolicy::Delivery)
            };
            let due = if by_delivery {
                *line.get_qty_delivered(env)?
            } else {
                ordered
            };
            let to_invoice = if confirmed {
                due - invoiced
            } else {
                Decimal::ZERO
            };
            let status = if !to_invoice.is_zero() {
                InvoiceStatus::ToInvoice
            } else if confirmed && invoiced >= ordered && !ordered.is_zero() {
                InvoiceStatus::Invoiced
            } else {
                InvoiceStatus::No
            };
            line.set_qty_invoiced(invoiced, env)?;
            line.set_qty_to_invoice(to_invoice, env)?;
            line.set_invoice_status(status, env)?;
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
                .get_option::<&Decimal>("product_uom_qty")
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
            .get_option::<&Decimal>("product_uom_qty")
            .is_some_and(|quantity| *quantity < Decimal::ZERO)
        {
            return Err("An order line's quantity is not negative".into());
        }
        const PRICED: [&str; 6] = [
            "product",
            "product_uom_qty",
            "price_unit",
            "discount",
            "taxes",
            "uom",
        ];
        let priced = values
            .fields
            .keys()
            .any(|field| PRICED.contains(&field.as_str()));
        if priced {
            for line in self {
                let order: SaleOrder<SingleId> = line.get_order(env)?;
                if order.is_state(env, SaleState::Cancel)? {
                    return Err(format!(
                        "{} is cancelled: its lines cannot change",
                        order.get_name(env)?
                    )
                    .into());
                }
                if values.contains_key("product") && order.is_state(env, SaleState::Sale)? {
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

    /// Lines invoiced already stay on their order.
    pub fn delete(&self, env: &mut Environment, sup: Super) -> Result<u32> {
        for line in self {
            if !line.get_qty_invoiced(env)?.is_zero() {
                return Err(format!(
                    "\"{}\" is invoiced: it stays on its order",
                    line.get_name(env)?.cloned().unwrap_or_default()
                )
                .into());
            }
        }
        sup.call(env)
    }
}
