use crate::models::extensions::ContactSale;
use crate::models::pricelist::{BaseProductPricelist, Pricelist};
use crate::models::sale_order_line::{BaseSaleOrderLine, InvoiceStatus, SaleOrderLine};
use account::models::{
    BaseAccountFiscalPosition, BaseAccountMove, BaseAccountPaymentTerm, ContactAccount,
    FiscalPosition, Move, MoveState, PaymentTerm,
};
use base::models::{BaseContact, BaseUsers, Contact};
use code_gen::{Model, erp_methods, selection};
use currency::models::{BaseCurrency, Currency};
use erp::Result;
use erp::environment::Environment;
use erp::serde_json::{Value, json};
use erp::types::field::Selection;
use erp::types::field::{
    Command, Decimal, FieldType, IdMode, MultipleIds, NaiveDate, Reference, SingleId, TimeDelta,
    Utc,
};
use erp::types::model::MapOfFields;
use sequence::models::Sequence;

#[selection]
pub enum SaleState {
    #[default]
    #[selection(label = "Quotation")]
    Draft,
    #[selection(label = "Quotation sent")]
    Sent,
    #[selection(label = "Sales order")]
    Sale,
    #[selection(label = "Cancelled")]
    Cancel,
}

/// A quotation, and once confirmed, a sales order: what a customer buys, at what price, and
/// how much of it has been invoiced.
#[derive(Model)]
#[erp(id = "sale_order", methods)]
#[allow(dead_code)]
pub struct SaleOrder<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Number", default = "New", index = "trigram")]
    name: String,
    #[erp(label = "Customer", required, ondelete = "restrict", tracking)]
    partner: Reference<BaseContact, SingleId>,
    #[erp(label = "Order date", tracking, index)]
    date_order: NaiveDate,
    #[erp(
        label = "Valid until",
        compute = "compute_validity_date",
        depends = ["date_order"],
        stored,
        editable
    )]
    validity_date: Option<NaiveDate>,
    #[erp(label = "Status", tracking, index)]
    state: SaleState,
    #[erp(
        ondelete = "restrict",
        compute = "compute_pricelist",
        depends = ["partner"],
        stored,
        editable
    )]
    pricelist: Reference<BaseProductPricelist, SingleId>,
    #[erp(
        label = "Payment terms",
        ondelete = "restrict",
        compute = "compute_payment_term",
        depends = ["partner"],
        stored,
        editable
    )]
    payment_term: Reference<BaseAccountPaymentTerm, SingleId>,
    #[erp(
        label = "Fiscal position",
        ondelete = "restrict",
        compute = "compute_fiscal_position",
        depends = ["partner"],
        stored,
        editable
    )]
    fiscal_position: Reference<BaseAccountFiscalPosition, SingleId>,
    #[erp(ondelete = "restrict", compute = "compute_currency", depends = ["pricelist"], stored)]
    currency: Reference<BaseCurrency, SingleId>,
    #[erp(label = "Customer reference", index = "trigram")]
    client_order_ref: Option<String>,
    #[erp(label = "Terms and conditions")]
    note: Option<String>,
    #[erp(label = "Salesperson", ondelete = "set_null")]
    user: Reference<BaseUsers, SingleId>,
    #[erp(label = "Order lines", inverse = "order", owned)]
    lines: Reference<BaseSaleOrderLine, MultipleIds>,
    #[erp(
        label = "Untaxed amount",
        compute = "compute_amounts",
        depends = ["lines.price_subtotal", "lines.price_total"],
        stored
    )]
    amount_untaxed: Decimal,
    #[erp(
        label = "Tax",
        compute = "compute_amounts",
        depends = ["lines.price_subtotal", "lines.price_total"],
        stored
    )]
    amount_tax: Decimal,
    #[erp(
        label = "Total",
        compute = "compute_amounts",
        depends = ["lines.price_subtotal", "lines.price_total"],
        stored
    )]
    amount_total: Decimal,
    #[erp(
        label = "Invoice status",
        compute = "compute_invoice_status",
        depends = ["lines.invoice_status", "state"],
        stored
    )]
    invoice_status: InvoiceStatus,
    #[erp(
        label = "Invoices",
        compute = "compute_invoices",
        depends = ["lines.invoice_lines"]
    )]
    invoices: Reference<BaseAccountMove, MultipleIds>,
}

#[erp_methods]
impl SaleOrder<SingleId> {
    pub fn is_state(&self, env: &mut Environment, state: SaleState) -> Result<bool> {
        Ok(self.get_state(env)?.key() == state.key())
    }

    /// The order's currency, the company's when none is set.
    pub fn currency_or_company(&self, env: &mut Environment) -> Result<Currency<SingleId>> {
        let currency: Currency<SingleId> = self.get_currency(env)?;
        if currency.is_empty() {
            Currency::of_company(env)
        } else {
            Ok(currency)
        }
    }

    /// The invoice of what is left to invoice on the order: a draft, or a credit note when more
    /// was invoiced than is due now. Empty when there is nothing to invoice.
    pub fn create_invoice(&self, env: &mut Environment) -> Result<Move<SingleId>> {
        if !self.is_state(env, SaleState::Sale)? {
            return Err(format!(
                "{} is not confirmed: confirm it before invoicing",
                self.get_name(env)?
            )
            .into());
        }
        let lines: SaleOrderLine<MultipleIds> = self.get_lines(env)?;
        let mut rows = Vec::new();
        let mut total = Decimal::ZERO;
        for line in &lines {
            let quantity = *line.get_qty_to_invoice(env)?;
            if quantity.is_zero() {
                continue;
            }
            rows.push((line.clone(), quantity));
            total += quantity * *line.get_price_unit(env)?;
        }
        if rows.is_empty() {
            return Ok(env.get_record(SingleId::empty()));
        }
        let refund = total < Decimal::ZERO;
        let mut invoice_lines = Vec::new();
        for (line, quantity) in rows {
            let mut values = MapOfFields::default();
            let product: product::models::Product<SingleId> = line.get_product(env)?;
            if let Some(product) = product.get_optional_id() {
                values.insert("product", product);
            }
            values.insert_option("name", line.get_name(env)?.cloned());
            values.insert("quantity", if refund { -quantity } else { quantity });
            let uom: uom::models::Uom<SingleId> = line.get_uom(env)?;
            if let Some(uom) = uom.get_optional_id() {
                values.insert("uom", uom);
            }
            values.insert("price_unit", *line.get_price_unit(env)?);
            values.insert("discount", *line.get_discount(env)?);
            let taxes: account::models::Tax<MultipleIds> = line.get_taxes(env)?;
            values.insert("taxes", FieldType::Refs(taxes.get_ids_ref().clone()));
            values.insert("sale_lines", FieldType::Refs(vec![line.get_id()]));
            values.insert("sequence", *line.get_sequence(env)?);
            invoice_lines.push(values);
        }
        let partner: Contact<SingleId> = self.get_partner(env)?;
        let mut invoice = MapOfFields::default();
        invoice.insert(
            "move_type",
            if refund { "out_refund" } else { "out_invoice" },
        );
        invoice.insert("partner", partner.get_id());
        invoice.insert("currency", self.currency_or_company(env)?.get_id());
        let reference = self
            .get_client_order_ref(env)?
            .cloned()
            .unwrap_or(self.get_name(env)?.clone());
        invoice.insert("reference", reference);
        let term: PaymentTerm<SingleId> = self.get_payment_term(env)?;
        if let Some(term) = term.get_optional_id() {
            invoice.insert("payment_term", term);
        }
        let position: FiscalPosition<SingleId> = self.get_fiscal_position(env)?;
        if let Some(position) = position.get_optional_id() {
            invoice.insert("fiscal_position", position);
        }
        invoice.insert_option("narration", self.get_note(env)?.cloned());
        invoice.insert_field_type(
            "invoice_lines",
            FieldType::Commands(vec![Command::Create(invoice_lines)]),
        );
        env.create_new_record_from_map(invoice)
    }
}

#[erp_methods]
impl SaleOrder<MultipleIds> {
    /// A quotation is valid a month from its date.
    pub fn compute_validity_date(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let date = *order.get_date_order(env)?;
            order.set_validity_date(Some(date + TimeDelta::days(30)), env)?;
        }
        Ok(())
    }

    /// The customer's pricelist.
    pub fn compute_pricelist(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let partner: Contact<SingleId> = order.get_partner(env)?;
            let pricelist: Pricelist<SingleId> = if partner.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                let env = &mut *env.sudo();
                let partner: ContactSale<SingleId> = env.get_record(partner.get_id().into());
                partner.get_pricelist(env)?
            };
            order.set_pricelist(&pricelist, env)?;
        }
        Ok(())
    }

    /// The customer's payment terms.
    pub fn compute_payment_term(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let partner: Contact<SingleId> = order.get_partner(env)?;
            let term: PaymentTerm<SingleId> = if partner.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                let env = &mut *env.sudo();
                let partner: ContactAccount<SingleId> = env.get_record(partner.get_id().into());
                partner.get_customer_payment_term(env)?
            };
            order.set_payment_term(&term, env)?;
        }
        Ok(())
    }

    /// The customer's fiscal position.
    pub fn compute_fiscal_position(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let partner: Contact<SingleId> = order.get_partner(env)?;
            let position: FiscalPosition<SingleId> = if partner.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                let env = &mut *env.sudo();
                let partner: ContactAccount<SingleId> = env.get_record(partner.get_id().into());
                partner.get_fiscal_position(env)?
            };
            order.set_fiscal_position(&position, env)?;
        }
        Ok(())
    }

    /// The pricelist's currency, else the company's.
    pub fn compute_currency(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let pricelist: Pricelist<SingleId> = order.get_pricelist(env)?;
            let mut currency: Currency<SingleId> = env.get_record(SingleId::empty());
            if !pricelist.is_empty() {
                currency = pricelist.get_currency(&mut env.sudo())?;
            }
            if currency.is_empty() {
                currency = Currency::of_company(env)?;
            }
            order.set_currency(&currency, env)?;
        }
        Ok(())
    }

    pub fn compute_amounts(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let lines: SaleOrderLine<MultipleIds> = order.get_lines(env)?;
            let untaxed: Decimal = lines.get_price_subtotal(env)?.into_iter().copied().sum();
            let total: Decimal = lines.get_price_total(env)?.into_iter().copied().sum();
            order.set_amount_untaxed(untaxed, env)?;
            order.set_amount_tax(total - untaxed, env)?;
            order.set_amount_total(total, env)?;
        }
        Ok(())
    }

    /// To invoice when a line is; invoiced when all lines are; nothing before confirmation.
    pub fn compute_invoice_status(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let status = if !order.is_state(env, SaleState::Sale)? {
                InvoiceStatus::No
            } else {
                let lines: SaleOrderLine<MultipleIds> = order.get_lines(env)?;
                let statuses: Vec<InvoiceStatus> = lines
                    .get_invoice_status(env)?
                    .into_iter()
                    .copied()
                    .collect();
                if statuses
                    .iter()
                    .any(|status| matches!(status, InvoiceStatus::ToInvoice))
                {
                    InvoiceStatus::ToInvoice
                } else if !statuses.is_empty()
                    && statuses
                        .iter()
                        .all(|status| matches!(status, InvoiceStatus::Invoiced))
                {
                    InvoiceStatus::Invoiced
                } else {
                    InvoiceStatus::No
                }
            };
            order.set_invoice_status(status, env)?;
        }
        Ok(())
    }

    /// The invoices and credit notes made from the order's lines.
    pub fn compute_invoices(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let lines: SaleOrderLine<MultipleIds> = order.get_lines(env)?;
            let mut invoices: Vec<u32> = Vec::new();
            let env = &mut *env.sudo();
            for line in &lines {
                let invoice_lines: account::models::InvoiceLine<MultipleIds> =
                    line.get_invoice_lines(env)?;
                for invoice_line in &invoice_lines {
                    let invoice: Move<SingleId> = invoice_line.get_move_id(env)?;
                    if !invoices.contains(&invoice.get_id()) {
                        invoices.push(invoice.get_id());
                    }
                }
            }
            let invoices: Move<MultipleIds> = Move::from_ids(invoices, env);
            order.set_invoices(&invoices, env)?;
        }
        Ok(())
    }

    /// A quotation starts dated today, and made by whoever creates it.
    pub fn default_get(
        env: &mut Environment,
        fields: Vec<String>,
        sup: Super,
    ) -> Result<MapOfFields> {
        let mut defaults = sup.call_with(fields.clone(), env)?;
        let today = Utc::now().date_naive();
        let asked = |name: &str| fields.iter().any(|field| field == name);
        if asked("date_order") {
            defaults.insert("date_order", today);
        }
        if asked("user")
            && let Some(uid) = env.uid()
        {
            defaults.insert("user", uid);
        }
        Ok(defaults)
    }

    /// A quotation is numbered when created, in the sequence of the year it is dated.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let mut values = values;
        for order in &mut values {
            let date = order
                .get_option::<&NaiveDate>("date_order")
                .copied()
                .unwrap_or_else(|| Utc::now().date_naive());
            if order
                .get_option::<&String>("name")
                .is_none_or(|name| name == "New")
            {
                order.insert(
                    "name",
                    Sequence::next_by_code(env, "sale.order".to_string(), date)?,
                );
            }
        }
        sup.call_with(values, env)
    }

    /// A confirmed order keeps its customer, currency and prices; a cancelled one stays as it
    /// was until set back to quotation.
    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        const LOCKED: [&str; 4] = ["partner", "pricelist", "currency", "fiscal_position"];
        let locked: Vec<&str> = values
            .fields
            .keys()
            .map(String::as_str)
            .filter(|field| LOCKED.contains(field))
            .collect();
        if !locked.is_empty() {
            for order in self {
                if order.is_state(env, SaleState::Sale)?
                    || order.is_state(env, SaleState::Cancel)?
                {
                    return Err(format!(
                        "{} is no longer a quotation: its {} cannot change",
                        order.get_name(env)?,
                        locked.join(", ")
                    )
                    .into());
                }
            }
        }
        sup.call_with(values, env)
    }

    /// Only quotations and cancelled orders are deleted; a confirmed order is cancelled.
    pub fn delete(&self, env: &mut Environment, sup: Super) -> Result<u32> {
        for order in self {
            if order.is_state(env, SaleState::Sale)? {
                return Err(format!(
                    "{} is confirmed: cancel it rather than delete it",
                    order.get_name(env)?
                )
                .into());
            }
        }
        sup.call(env)
    }

    /// Mark the quotations as sent to the customer.
    #[erp(rpc)]
    pub fn action_quotation_send(&self, env: &mut Environment) -> Result<bool> {
        for order in self {
            if !order.is_state(env, SaleState::Draft)? && !order.is_state(env, SaleState::Sent)? {
                return Err(format!("{} is no longer a quotation", order.get_name(env)?).into());
            }
            order.set_state(SaleState::Sent, env)?;
        }
        Ok(true)
    }

    /// Confirm the quotations: they become sales orders, dated today.
    #[erp(rpc)]
    pub fn action_confirm(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for order in self {
                if !order.is_state(env, SaleState::Draft)?
                    && !order.is_state(env, SaleState::Sent)?
                {
                    return Err(format!(
                        "{} is not a quotation: it cannot be confirmed",
                        order.get_name(env)?
                    )
                    .into());
                }
                let lines: SaleOrderLine<MultipleIds> = order.get_lines(env)?;
                if lines.get_ids_ref().is_empty() {
                    return Err(format!("{} has no line to confirm", order.get_name(env)?).into());
                }
                order.set_state(SaleState::Sale, env)?;
                Self::from_ids(vec![order.get_id()], env).on_confirmed(env)?;
            }
            Ok(true)
        })
    }

    /// Cancel the orders: their draft invoices are cancelled with them; one invoiced and posted
    /// is credited first.
    #[erp(rpc)]
    pub fn action_cancel(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for order in self {
                let invoices: Move<MultipleIds> = order.get_invoices(env)?;
                let mut drafts = Vec::new();
                for invoice in &invoices {
                    let invoice_state = *invoice.get_state(&mut env.sudo())?;
                    match invoice_state {
                        MoveState::Posted => {
                            let invoice_name = invoice.get_name(env)?.clone();
                            return Err(format!(
                                "{} is invoiced by {invoice_name}: credit the invoice before cancelling the order",
                                order.get_name(env)?
                            )
                            .into());
                        }
                        MoveState::Draft => drafts.push(invoice.get_id()),
                        _ => {}
                    }
                }
                if !drafts.is_empty() {
                    Move::<MultipleIds>::from_ids(drafts, env).button_cancel(env)?;
                }
                order.set_state(SaleState::Cancel, env)?;
                Self::from_ids(vec![order.get_id()], env).on_cancelled(env)?;
            }
            Ok(true)
        })
    }

    /// Set cancelled orders back to quotation.
    #[erp(rpc)]
    pub fn action_draft(&self, env: &mut Environment) -> Result<bool> {
        for order in self {
            if !order.is_state(env, SaleState::Cancel)? {
                return Err(format!("{} is not cancelled", order.get_name(env)?).into());
            }
            order.set_state(SaleState::Draft, env)?;
        }
        Ok(true)
    }

    /// Invoice what is left to invoice on the orders, and open the invoice.
    #[erp(rpc)]
    pub fn action_create_invoice(&self, env: &mut Environment) -> Result<Value> {
        let mut made = Vec::new();
        env.savepoint(|env| {
            for order in self {
                let invoice = order.create_invoice(env)?;
                if !invoice.is_empty() {
                    made.push(invoice);
                }
            }
            Ok(())
        })?;
        match made.as_slice() {
            [] => Err("There is nothing to invoice".into()),
            [single] => {
                let action = single.action_xml_id(env)?;
                Ok(json!({"type": "open", "action": action, "id": single.get_id()}))
            }
            _ => Ok(json!({"type": "reload"})),
        }
    }

    /// What follows a confirmation; inventory delivers what is sold.
    pub fn on_confirmed(&self, _env: &mut Environment) -> Result<()> {
        Ok(())
    }

    /// What follows a cancellation; inventory cancels what was to be delivered.
    pub fn on_cancelled(&self, _env: &mut Environment) -> Result<()> {
        Ok(())
    }
}
