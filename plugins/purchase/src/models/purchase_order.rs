use crate::models::purchase_order_line::{BasePurchaseOrderLine, BillStatus, PurchaseOrderLine};
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
use erp::types::field::{
    Command, Decimal, FieldType, IdMode, MultipleIds, NaiveDate, Reference, Selection, SingleId,
    Utc,
};
use erp::types::model::MapOfFields;
use sequence::models::Sequence;

#[selection]
pub enum PurchaseState {
    #[default]
    #[selection(label = "Request for quotation")]
    Draft,
    #[selection(label = "Request sent")]
    Sent,
    #[selection(label = "Purchase order")]
    Purchase,
    #[selection(label = "Cancelled")]
    Cancel,
}

/// A request for quotation, and once confirmed, a purchase order: what is bought from a
/// vendor, at what price, and how much of it is billed.
#[derive(Model)]
#[erp(id = "purchase_order", methods)]
#[allow(dead_code)]
pub struct PurchaseOrder<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Number", default = "New")]
    name: String,
    #[erp(label = "Vendor", required, ondelete = "restrict", tracking)]
    partner: Reference<BaseContact, SingleId>,
    #[erp(label = "Vendor reference")]
    partner_ref: Option<String>,
    #[erp(label = "Order date", tracking)]
    date_order: NaiveDate,
    #[erp(label = "Expected arrival")]
    date_planned: Option<NaiveDate>,
    #[erp(label = "Status", tracking)]
    state: PurchaseState,
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
    #[erp(ondelete = "restrict")]
    currency: Reference<BaseCurrency, SingleId>,
    #[erp(label = "Notes")]
    notes: Option<String>,
    #[erp(label = "Buyer", ondelete = "set_null")]
    user: Reference<BaseUsers, SingleId>,
    #[erp(label = "Order lines", inverse = "order", owned)]
    lines: Reference<BasePurchaseOrderLine, MultipleIds>,
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
        label = "Billing status",
        compute = "compute_bill_status",
        depends = ["lines.bill_status", "state"],
        stored
    )]
    bill_status: BillStatus,
    #[erp(label = "Bills", compute = "compute_bills", depends = ["lines.bill_lines"])]
    bills: Reference<BaseAccountMove, MultipleIds>,
}

impl PurchaseOrder<SingleId> {
    pub fn is_state(&self, env: &mut Environment, state: PurchaseState) -> Result<bool> {
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

    /// The vendor bill of what is left to bill on the order: a draft, or a vendor credit note
    /// when more was billed than is due now. Empty when there is nothing to bill.
    pub fn create_bill(&self, env: &mut Environment) -> Result<Move<SingleId>> {
        if !self.is_state(env, PurchaseState::Purchase)? {
            return Err(format!(
                "{} is not confirmed: confirm it before billing",
                self.get_name(env)?
            )
            .into());
        }
        let lines: PurchaseOrderLine<MultipleIds> = self.get_lines(env)?;
        let mut rows = Vec::new();
        let mut total = Decimal::ZERO;
        for line in &lines {
            let quantity = *line.get_qty_to_bill(env)?;
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
        let mut bill_lines = Vec::new();
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
            values.insert("purchase_lines", FieldType::Refs(vec![line.get_id()]));
            values.insert("sequence", *line.get_sequence(env)?);
            bill_lines.push(values);
        }
        let partner: Contact<SingleId> = self.get_partner(env)?;
        let mut bill = MapOfFields::default();
        bill.insert("move_type", if refund { "in_refund" } else { "in_invoice" });
        bill.insert("partner", partner.get_id());
        bill.insert("currency", self.currency_or_company(env)?.get_id());
        let reference = self
            .get_partner_ref(env)?
            .cloned()
            .unwrap_or(self.get_name(env)?.clone());
        bill.insert("reference", reference);
        let term: PaymentTerm<SingleId> = self.get_payment_term(env)?;
        if let Some(term) = term.get_optional_id() {
            bill.insert("payment_term", term);
        }
        let position: FiscalPosition<SingleId> = self.get_fiscal_position(env)?;
        if let Some(position) = position.get_optional_id() {
            bill.insert("fiscal_position", position);
        }
        bill.insert_field_type(
            "invoice_lines",
            FieldType::Commands(vec![Command::Create(bill_lines)]),
        );
        env.create_new_record_from_map(bill)
    }
}

#[erp_methods]
impl PurchaseOrder<MultipleIds> {
    /// The vendor's payment terms.
    pub fn compute_payment_term(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let partner: Contact<SingleId> = order.get_partner(env)?;
            let term: PaymentTerm<SingleId> = if partner.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                let env = &mut *env.sudo();
                let partner: ContactAccount<SingleId> = env.get_record(partner.get_id().into());
                partner.get_supplier_payment_term(env)?
            };
            order.set_payment_term(&term, env)?;
        }
        Ok(())
    }

    /// The vendor's fiscal position.
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

    pub fn compute_amounts(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let lines: PurchaseOrderLine<MultipleIds> = order.get_lines(env)?;
            let untaxed: Decimal = lines.get_price_subtotal(env)?.into_iter().copied().sum();
            let total: Decimal = lines.get_price_total(env)?.into_iter().copied().sum();
            order.set_amount_untaxed(untaxed, env)?;
            order.set_amount_tax(total - untaxed, env)?;
            order.set_amount_total(total, env)?;
        }
        Ok(())
    }

    /// To bill when a line is; billed when all lines are; nothing before confirmation.
    pub fn compute_bill_status(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let status = if !order.is_state(env, PurchaseState::Purchase)? {
                BillStatus::No
            } else {
                let lines: PurchaseOrderLine<MultipleIds> = order.get_lines(env)?;
                let statuses: Vec<BillStatus> =
                    lines.get_bill_status(env)?.into_iter().copied().collect();
                if statuses
                    .iter()
                    .any(|status| matches!(status, BillStatus::ToBill))
                {
                    BillStatus::ToBill
                } else if !statuses.is_empty()
                    && statuses
                        .iter()
                        .all(|status| matches!(status, BillStatus::Billed))
                {
                    BillStatus::Billed
                } else {
                    BillStatus::No
                }
            };
            order.set_bill_status(status, env)?;
        }
        Ok(())
    }

    /// The vendor bills and credit notes made from the order's lines.
    pub fn compute_bills(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let lines: PurchaseOrderLine<MultipleIds> = order.get_lines(env)?;
            let mut bills: Vec<u32> = Vec::new();
            let env = &mut *env.sudo();
            for line in &lines {
                let bill_lines: account::models::InvoiceLine<MultipleIds> =
                    line.get_bill_lines(env)?;
                for bill_line in &bill_lines {
                    let bill: Move<SingleId> = bill_line.get_move_id(env)?;
                    if !bills.contains(&bill.get_id()) {
                        bills.push(bill.get_id());
                    }
                }
            }
            let bills: Move<MultipleIds> = Move::from_ids(bills, env);
            order.set_bills(&bills, env)?;
        }
        Ok(())
    }

    /// A request is numbered when created, dated today, bought in the company's currency by its
    /// creator.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let today = Utc::now().date_naive();
        let company_currency = Currency::of_company(env)?;
        let mut values = values;
        for order in &mut values {
            if order.get_option::<&NaiveDate>("date_order").is_none() {
                order.insert("date_order", today);
            }
            let date = *order
                .get_option::<&NaiveDate>("date_order")
                .expect("just set");
            if order
                .get_option::<&String>("name")
                .is_none_or(|name| name == "New")
            {
                order.insert("name", Sequence::next_by_code(env, "purchase.order", date)?);
            }
            if order
                .get_option::<&u32>("currency")
                .is_none_or(|id| *id == 0)
                && let Some(currency) = company_currency.get_optional_id()
            {
                order.insert("currency", currency);
            }
            if !order.contains_key("user")
                && let Some(uid) = env.uid()
            {
                order.insert("user", uid);
            }
        }
        sup.call_with(values, env)
    }

    /// A confirmed order keeps its vendor, currency and fiscal position.
    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        const LOCKED: [&str; 3] = ["partner", "currency", "fiscal_position"];
        let locked: Vec<&str> = values
            .fields
            .keys()
            .map(String::as_str)
            .filter(|field| LOCKED.contains(field))
            .collect();
        if !locked.is_empty() {
            for order in self {
                if order.is_state(env, PurchaseState::Purchase)?
                    || order.is_state(env, PurchaseState::Cancel)?
                {
                    return Err(format!(
                        "{} is no longer a request: its {} cannot change",
                        order.get_name(env)?,
                        locked.join(", ")
                    )
                    .into());
                }
            }
        }
        sup.call_with(values, env)
    }

    /// Only requests and cancelled orders are deleted; a confirmed order is cancelled.
    pub fn delete(&self, env: &mut Environment, sup: Super) -> Result<u32> {
        for order in self {
            if order.is_state(env, PurchaseState::Purchase)? {
                return Err(format!(
                    "{} is confirmed: cancel it rather than delete it",
                    order.get_name(env)?
                )
                .into());
            }
        }
        sup.call(env)
    }

    /// Mark the requests as sent to the vendor.
    #[erp(rpc)]
    pub fn action_rfq_send(&self, env: &mut Environment) -> Result<bool> {
        for order in self {
            if !order.is_state(env, PurchaseState::Draft)?
                && !order.is_state(env, PurchaseState::Sent)?
            {
                return Err(format!("{} is no longer a request", order.get_name(env)?).into());
            }
            order.set_state(PurchaseState::Sent, env)?;
        }
        Ok(true)
    }

    /// Confirm the requests: they become purchase orders.
    #[erp(rpc)]
    pub fn button_confirm(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for order in self {
                if !order.is_state(env, PurchaseState::Draft)?
                    && !order.is_state(env, PurchaseState::Sent)?
                {
                    return Err(format!(
                        "{} is not a request: it cannot be confirmed",
                        order.get_name(env)?
                    )
                    .into());
                }
                let lines: PurchaseOrderLine<MultipleIds> = order.get_lines(env)?;
                if lines.get_ids_ref().is_empty() {
                    return Err(format!("{} has no line to confirm", order.get_name(env)?).into());
                }
                order.set_state(PurchaseState::Purchase, env)?;
                Self::from_ids(vec![order.get_id()], env).on_confirmed(env)?;
            }
            Ok(true)
        })
    }

    /// Cancel the orders with their draft bills; one billed and posted is credited first.
    #[erp(rpc)]
    pub fn button_cancel(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for order in self {
                let bills: Move<MultipleIds> = order.get_bills(env)?;
                let mut drafts = Vec::new();
                for bill in &bills {
                    let state = *bill.get_state(&mut env.sudo())?;
                    match state {
                        MoveState::Posted => {
                            let bill_name = bill.get_name(env)?.clone();
                            return Err(format!(
                                "{} is billed by {bill_name}: credit the bill before cancelling the order",
                                order.get_name(env)?
                            )
                            .into());
                        }
                        MoveState::Draft => drafts.push(bill.get_id()),
                        _ => {}
                    }
                }
                if !drafts.is_empty() {
                    Move::<MultipleIds>::from_ids(drafts, env).button_cancel(env)?;
                }
                order.set_state(PurchaseState::Cancel, env)?;
                Self::from_ids(vec![order.get_id()], env).on_cancelled(env)?;
            }
            Ok(true)
        })
    }

    /// Set cancelled orders back to request for quotation.
    #[erp(rpc)]
    pub fn button_draft(&self, env: &mut Environment) -> Result<bool> {
        for order in self {
            if !order.is_state(env, PurchaseState::Cancel)? {
                return Err(format!("{} is not cancelled", order.get_name(env)?).into());
            }
            order.set_state(PurchaseState::Draft, env)?;
        }
        Ok(true)
    }

    /// Bill what is left to bill on the orders, and open the bill.
    #[erp(rpc)]
    pub fn action_create_bill(&self, env: &mut Environment) -> Result<Value> {
        let mut made = Vec::new();
        env.savepoint(|env| {
            for order in self {
                let bill = order.create_bill(env)?;
                if !bill.is_empty() {
                    made.push(bill);
                }
            }
            Ok(())
        })?;
        match made.as_slice() {
            [] => Err("There is nothing to bill".into()),
            [single] => {
                let action = single.action_xml_id(env)?;
                Ok(json!({"type": "open", "action": action, "id": single.get_id()}))
            }
            _ => Ok(json!({"type": "reload"})),
        }
    }

    /// What follows a confirmation; inventory receives what is bought.
    pub fn on_confirmed(&self, _env: &mut Environment) -> Result<()> {
        Ok(())
    }

    /// What follows a cancellation; inventory cancels what was to be received.
    pub fn on_cancelled(&self, _env: &mut Environment) -> Result<()> {
        Ok(())
    }
}
