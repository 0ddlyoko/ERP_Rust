use crate::models::account::Account;
use crate::models::company::CompanyAccount;
use crate::models::contact::ContactAccount;
use crate::models::fiscal_position::{BaseAccountFiscalPosition, FiscalPosition};
use crate::models::invoice_line::{BaseAccountInvoiceLine, InvoiceLine};
use crate::models::journal::{BaseAccountJournal, Journal, JournalType};
use crate::models::move_line::{BaseAccountMoveLine, LineKind, MoveLine};
use crate::models::payment::{PartnerType, Payment};
use crate::models::payment_term::{BaseAccountPaymentTerm, PaymentTerm};
use crate::models::tax::TaxDocument;
use base::models::{BaseContact, Contact};
use code_gen::{Model, erp_methods, selection};
use currency::models::{BaseCurrency, Currency};
use erp::Result;
use erp::environment::Environment;
use erp::serde_json::{Value, json};
use erp::types::field::Selection;
use erp::types::field::{
    Command, Decimal, FieldType, IdMode, MultipleIds, NaiveDate, Reference, SingleId, Utc,
};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use std::collections::BTreeMap;

#[selection]
pub enum MoveType {
    #[default]
    #[selection(label = "Journal entry")]
    Entry,
    #[selection(label = "Customer invoice")]
    OutInvoice,
    #[selection(label = "Customer credit note")]
    OutRefund,
    #[selection(label = "Vendor bill")]
    InInvoice,
    #[selection(label = "Vendor credit note")]
    InRefund,
}

impl MoveType {
    pub fn is_invoice(self) -> bool {
        !matches!(self, MoveType::Entry)
    }

    pub fn is_sale(self) -> bool {
        matches!(self, MoveType::OutInvoice | MoveType::OutRefund)
    }

    pub fn is_refund(self) -> bool {
        matches!(self, MoveType::OutRefund | MoveType::InRefund)
    }

    /// The sign of the invoice's lines of products and taxes: credited on a customer invoice,
    /// debited on a vendor bill, the other way round on their credit notes.
    pub fn product_sign(self) -> Decimal {
        match self {
            MoveType::OutInvoice | MoveType::InRefund => -Decimal::ONE,
            _ => Decimal::ONE,
        }
    }

    /// The credit note of this kind of document.
    pub fn reversed(self) -> MoveType {
        match self {
            MoveType::OutInvoice => MoveType::OutRefund,
            MoveType::OutRefund => MoveType::OutInvoice,
            MoveType::InInvoice => MoveType::InRefund,
            MoveType::InRefund => MoveType::InInvoice,
            other => other,
        }
    }
}

#[selection]
pub enum MoveState {
    #[default]
    #[selection(label = "Draft")]
    Draft,
    #[selection(label = "Posted")]
    Posted,
    #[selection(label = "Cancelled")]
    Cancel,
}

#[selection]
pub enum PaymentState {
    #[default]
    #[selection(label = "Not paid")]
    NotPaid,
    #[selection(label = "Partially paid")]
    Partial,
    #[selection(label = "Paid")]
    Paid,
    #[selection(label = "Reversed")]
    Reversed,
}

/// A journal entry: an invoice, a vendor bill, a credit note, or an entry made by hand — its
/// journal items balancing debits with credits once posted.
#[derive(Model)]
#[erp(id = "account_move", order = "date desc, name desc, id desc", methods)]
#[allow(dead_code)]
pub struct Move<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Number", default = "/", tracking, index = "trigram")]
    name: String,
    #[erp(label = "Type", index)]
    move_type: MoveType,
    #[erp(label = "Status", tracking, index)]
    state: MoveState,
    #[erp(
        required,
        ondelete = "restrict",
        tracking,
        domain = "move_type === 'entry' ? [] : [['journal_type', '=', move_type === 'out_invoice' || move_type === 'out_refund' ? 'sale' : 'purchase']]"
    )]
    journal: Reference<BaseAccountJournal, SingleId>,
    #[erp(label = "Accounting date", tracking, index)]
    date: NaiveDate,
    #[erp(label = "Invoice date", tracking)]
    invoice_date: Option<NaiveDate>,
    #[erp(
        label = "Due date",
        compute = "compute_invoice_date_due",
        depends = ["invoice_date", "date", "payment_term", "lines.date_maturity"],
        stored
    )]
    invoice_date_due: Option<NaiveDate>,
    #[erp(ondelete = "restrict", tracking)]
    partner: Reference<BaseContact, SingleId>,
    #[erp(ondelete = "restrict", tracking)]
    currency: Reference<BaseCurrency, SingleId>,
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
    #[erp(
        label = "Reference",
        description = "The supplier's number, or the customer's order",
        index = "trigram"
    )]
    reference: Option<String>,
    #[erp(label = "Payment reference", index = "trigram")]
    payment_reference: Option<String>,
    #[erp(label = "Terms and conditions")]
    narration: Option<String>,
    #[erp(label = "Invoice lines", inverse = "move_id", owned)]
    invoice_lines: Reference<BaseAccountInvoiceLine, MultipleIds>,
    #[erp(label = "Journal items", inverse = "move_id", owned)]
    lines: Reference<BaseAccountMoveLine, MultipleIds>,
    #[erp(
        label = "Untaxed amount",
        compute = "compute_amounts",
        depends = ["invoice_lines.price_subtotal", "invoice_lines.price_total", "lines.debit", "move_type"],
        stored
    )]
    amount_untaxed: Decimal,
    #[erp(
        label = "Tax",
        compute = "compute_amounts",
        depends = ["invoice_lines.price_subtotal", "invoice_lines.price_total", "lines.debit", "move_type"],
        stored
    )]
    amount_tax: Decimal,
    #[erp(
        label = "Total",
        compute = "compute_amounts",
        depends = ["invoice_lines.price_subtotal", "invoice_lines.price_total", "lines.debit", "move_type"],
        stored
    )]
    amount_total: Decimal,
    #[erp(
        label = "Amount due",
        compute = "compute_residual",
        depends = ["lines.amount_residual", "lines.amount_residual_currency", "state", "amount_total"],
        stored
    )]
    amount_residual: Decimal,
    #[erp(
        label = "Payment status",
        compute = "compute_residual",
        depends = ["lines.amount_residual", "lines.amount_residual_currency", "state", "amount_total"],
        stored,
        tracking
    )]
    payment_state: PaymentState,
    #[erp(
        label = "Total (company currency)",
        compute = "compute_signed",
        depends = ["lines.balance", "move_type", "state"],
        stored
    )]
    amount_total_signed: Decimal,
    #[erp(label = "Reversal of", ondelete = "set_null")]
    reversed_entry: Reference<BaseAccountMove, SingleId>,
    #[erp(label = "Reversals", inverse = "reversed_entry")]
    reversals: Reference<BaseAccountMove, MultipleIds>,
    #[erp(label = "Reason of the reversal")]
    reversal_reason: Option<String>,
}

#[erp_methods]
impl Move<SingleId> {
    pub fn is_draft(&self, env: &mut Environment) -> Result<bool> {
        Ok(matches!(*self.get_state(env)?, MoveState::Draft))
    }

    pub fn is_posted(&self, env: &mut Environment) -> Result<bool> {
        Ok(matches!(*self.get_state(env)?, MoveState::Posted))
    }

    /// Whether the entry is a customer invoice or credit note: priced and taxed for sales.
    pub fn is_sale_document(&self, env: &mut Environment) -> Result<bool> {
        Ok(self.get_move_type(env)?.is_sale())
    }

    /// The entry's currency, the company's when none is set.
    pub fn currency_or_company(&self, env: &mut Environment) -> Result<Currency<SingleId>> {
        let currency: Currency<SingleId> = self.get_currency(env)?;
        if currency.is_empty() {
            Currency::of_company(env)
        } else {
            Ok(currency)
        }
    }

    /// The account the partner owes or is owed on: its own, else the company's.
    pub fn counterpart_account(&self, env: &mut Environment) -> Result<Account<SingleId>> {
        let sale = self.is_sale_document(env)?;
        let partner: Contact<SingleId> = self.get_partner(env)?;
        let env = &mut *env.sudo();
        let mut account: Account<SingleId> = env.get_record(SingleId::empty());
        if !partner.is_empty() {
            let partner: ContactAccount<SingleId> = env.get_record(partner.get_id().into());
            account = if sale {
                partner.get_account_receivable(env)?
            } else {
                partner.get_account_payable(env)?
            };
        }
        if account.is_empty() {
            let company = CompanyAccount::current(env)?;
            account = company.required_account(
                env,
                if sale {
                    "receivable".to_string()
                } else {
                    "payable".to_string()
                },
            )?;
        }
        Ok(account)
    }

    /// The journal items of the invoice, as its lines, taxes and terms make them: products and
    /// taxes on one side, what the partner owes on the other, in installments.
    fn invoice_items(&self, env: &mut Environment) -> Result<Vec<MapOfFields>> {
        let move_type = *self.get_move_type(env)?;
        let sign = move_type.product_sign();
        let document = if move_type.is_refund() {
            TaxDocument::Refund
        } else {
            TaxDocument::Invoice
        };
        let currency = self.currency_or_company(env)?;
        let company_currency = Currency::of_company(env)?;
        let date = *self.get_date(env)?;
        let invoice_date = self.get_invoice_date(env)?.copied().unwrap_or(date);
        let partner: Contact<SingleId> = self.get_partner(env)?;
        let to_company = |env: &mut Environment, amount: Decimal| -> Result<Decimal> {
            currency.convert(env, amount, company_currency.clone(), date)
        };

        let mut items = Vec::new();
        let mut taxes: BTreeMap<TaxItemKey, (Decimal, Decimal)> = BTreeMap::new();
        let mut total_currency = Decimal::ZERO;
        let lines: InvoiceLine<MultipleIds> = self.get_invoice_lines(env)?;
        for line in &lines {
            let result = line.taxed(env, document)?;
            let account: Account<SingleId> = line.get_account(env)?;
            if account.is_empty() {
                return Err(format!(
                    "The line \"{}\" has no account",
                    line.get_name(env)?.cloned().unwrap_or_default()
                )
                .into());
            }
            let mut base_tags = Vec::new();
            for tax in &result.taxes {
                let tax: crate::models::tax::Tax<SingleId> = env.get_record(tax.tax.into());
                base_tags.extend(tax.spec(env, document)?.base_tags);
            }
            base_tags.sort_unstable();
            base_tags.dedup();
            let amount_currency = sign * result.subtotal;
            let balance = to_company(env, amount_currency)?;
            let product: product::models::Product<SingleId> = line.get_product(env)?;
            let mut item = MapOfFields::default();
            item.insert("account", account.get_id());
            item.insert("name", line.get_name(env)?.cloned().unwrap_or_default());
            item.insert("display_type", LineKind::Product);
            item.insert("quantity", *line.get_quantity(env)?);
            if let Some(product) = product.get_optional_id() {
                item.insert("product", product);
            }
            item.insert(
                "taxes",
                FieldType::Refs(result.taxes.iter().map(|tax| tax.tax).collect()),
            );
            item.insert("tax_tags", FieldType::Refs(base_tags));
            put_amounts(&mut item, balance, amount_currency);
            items.push(item);
            total_currency += result.total;
            for tax in result.taxes {
                for (repartition, share) in tax.shares {
                    let key = (
                        tax.tax,
                        repartition.account,
                        repartition.tags.clone(),
                        account.get_id(),
                    );
                    let entry = taxes.entry(key).or_default();
                    entry.0 += share;
                    entry.1 += tax.base;
                }
            }
        }
        for ((tax, account, tags, line_account), (amount, base)) in taxes {
            if amount.is_zero() {
                continue;
            }
            let tax_record: crate::models::tax::Tax<SingleId> = env.get_record(tax.into());
            let label = {
                let env = &mut *env.sudo();
                tax_record
                    .get_description(env)?
                    .cloned()
                    .unwrap_or(tax_record.get_name(env)?.clone())
            };
            let amount_currency = sign * amount;
            let balance = to_company(env, amount_currency)?;
            let mut item = MapOfFields::default();
            item.insert("account", account.unwrap_or(line_account));
            item.insert("name", label);
            item.insert("display_type", LineKind::Tax);
            item.insert("tax_line", tax);
            item.insert("tax_base_amount", to_company(env, sign * base)?.abs());
            item.insert("tax_tags", FieldType::Refs(tags));
            put_amounts(&mut item, balance, amount_currency);
            items.push(item);
        }

        let counterpart = self.counterpart_account(env)?;
        let term: PaymentTerm<SingleId> = self.get_payment_term(env)?;
        let rounding = *currency.get_rounding(&mut env.sudo())?;
        let installments = term.compute(env, total_currency, invoice_date, rounding)?;
        let balance_so_far: Decimal = items
            .iter()
            .map(|item| {
                item.get_option::<&Decimal>("debit")
                    .copied()
                    .unwrap_or_default()
                    - item
                        .get_option::<&Decimal>("credit")
                        .copied()
                        .unwrap_or_default()
            })
            .sum();
        let count = installments.len();
        let mut placed = Decimal::ZERO;
        for (index, (due, amount)) in installments.into_iter().enumerate() {
            let amount_currency = -sign * amount;
            // The last installment takes what conversion rounding left, so the entry balances.
            let balance = if index + 1 == count {
                -balance_so_far - placed
            } else {
                to_company(env, amount_currency)?
            };
            placed += balance;
            let mut item = MapOfFields::default();
            item.insert("account", counterpart.get_id());
            item.insert(
                "name",
                self.get_payment_reference(env)?
                    .cloned()
                    .unwrap_or_default(),
            );
            item.insert("display_type", LineKind::PaymentTerm);
            item.insert("date_maturity", due);
            put_amounts(&mut item, balance, amount_currency);
            items.push(item);
        }
        for item in &mut items {
            item.insert("currency", currency.get_id());
            if let Some(partner) = partner.get_optional_id() {
                item.insert("partner", partner);
            }
        }
        Ok(items)
    }

    /// Debits and credits of the entry, in the company's currency.
    pub fn totals(&self, env: &mut Environment) -> Result<(Decimal, Decimal)> {
        let lines: MoveLine<MultipleIds> = self.get_lines(env)?;
        let debit = lines.get_debit(env)?.into_iter().copied().sum();
        let credit = lines.get_credit(env)?.into_iter().copied().sum();
        Ok((debit, credit))
    }

    /// The action showing entries of this type, to open them on.
    pub fn action_xml_id(&self, env: &mut Environment) -> Result<&'static str> {
        Ok(match *self.get_move_type(env)? {
            MoveType::OutInvoice => "account.action_move_out_invoice",
            MoveType::OutRefund => "account.action_move_out_refund",
            MoveType::InInvoice => "account.action_move_in_invoice",
            MoveType::InRefund => "account.action_move_in_refund",
            _ => "account.action_move_journal_entries",
        })
    }

    /// Whether every payment of the invoice is one of its own credit notes.
    fn settled_by_reversal(&self, env: &mut Environment) -> Result<bool> {
        let env = &mut *env.sudo();
        let reversals: Move<MultipleIds> = self.get_reversals(env)?;
        if reversals.get_ids_ref().is_empty() {
            return Ok(false);
        }
        let lines: MoveLine<MultipleIds> = self.get_lines(env)?;
        let group = lines.matched_group(env)?;
        for id in group {
            let line: MoveLine<SingleId> = env.get_record(id.into());
            let owner: Move<SingleId> = line.get_move_id(env)?;
            if owner.get_id() != self.get_id() && !reversals.get_ids_ref().contains(&owner.get_id())
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Remove the journal items of an invoice back to draft: they are made again at posting.
    fn clear_items(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        let lines: MoveLine<MultipleIds> = self.get_lines(env)?;
        if !lines.get_ids_ref().is_empty() {
            env.delete(
                "account_move_line",
                &MultipleIds::from(lines.get_ids_ref().clone()),
            )?;
        }
        Ok(())
    }

    fn post_one(&self, env: &mut Environment) -> Result<()> {
        if !self.is_draft(env)? {
            return Err(format!(
                "{} is not a draft: it cannot be posted",
                self.get_name(env)?
            )
            .into());
        }
        let move_type = *self.get_move_type(env)?;
        let today = Utc::now().date_naive();
        if move_type.is_invoice() {
            let partner: Contact<SingleId> = self.get_partner(env)?;
            if partner.is_empty() {
                return Err(format!(
                    "The {} needs a {} before it is posted",
                    move_type_label(move_type),
                    if move_type.is_sale() {
                        "customer"
                    } else {
                        "vendor"
                    }
                )
                .into());
            }
            let lines: InvoiceLine<MultipleIds> = self.get_invoice_lines(env)?;
            if lines.get_ids_ref().is_empty() {
                return Err(
                    format!("The {} has no line to post", move_type_label(move_type)).into(),
                );
            }
            if self.get_invoice_date(env)?.is_none() {
                self.set_invoice_date(Some(today), env)?;
            }
            let invoice_date = *self.get_invoice_date(env)?.expect("just set");
            self.set_date(invoice_date, env)?;
            if *self.get_amount_total(env)? < Decimal::ZERO {
                return Err(format!(
                    "The total of the {} is negative: make a credit note instead",
                    move_type_label(move_type)
                )
                .into());
            }
        }
        let date = *self.get_date(env)?;
        let company = CompanyAccount::current(env)?;
        company.check_lock(env, date)?;
        let journal: Journal<SingleId> = self.get_journal(env)?;
        if journal.is_empty() {
            return Err("An entry needs a journal".into());
        }
        if self.get_name(env)? == "/" {
            let numbering = journal.numbering(env, move_type.is_refund())?;
            if numbering.is_empty() {
                return Err("The journal has no numbering".into());
            }
            let name = numbering.next(env, date)?;
            let journal_id = journal.get_id();
            let taken = env.sudo().count(
                "account_move",
                &make_domain!([("journal", "=", journal_id), ("name", "=", name.clone())]),
            )?;
            if taken > 0 {
                return Err(format!("The number {name} is already used in this journal").into());
            }
            self.set_name(name, env)?;
        }
        if move_type.is_invoice() {
            self.clear_items(env)?;
            Move::<MultipleIds>::from_ids(vec![self.get_id()], env)
                .assign_payment_reference(env)?;
            let mut items = self.invoice_items(env)?;
            for item in &mut items {
                item.insert("move_id", self.get_id());
            }
            let _: MoveLine<MultipleIds> = env.sudo().create_new_records_from_maps(items)?;
        }
        let lines: MoveLine<MultipleIds> = self.get_lines(env)?;
        if lines.get_ids_ref().len() < 2 {
            return Err(format!("{} needs at least two journal items", self.get_name(env)?).into());
        }
        let (debit, credit) = self.totals(env)?;
        if debit != credit {
            return Err(format!(
                "{} does not balance: {debit} debited, {credit} credited",
                self.get_name(env)?
            )
            .into());
        }
        if debit.is_zero() && !move_type.is_invoice() {
            return Err(format!("{} records nothing", self.get_name(env)?).into());
        }
        self.set_state(MoveState::Posted, &mut env.sudo())?;
        Ok(())
    }

    /// The reverse of this entry: a draft credit note of an invoice, its lines copied; the
    /// posted mirror of an entry made by hand.
    pub fn reverse_one(
        &self,
        env: &mut Environment,
        date: Option<NaiveDate>,
        reason: Option<String>,
    ) -> Result<Move<SingleId>> {
        if !self.is_posted(env)? {
            return Err(format!(
                "{} is not posted: there is nothing to reverse",
                self.get_name(env)?
            )
            .into());
        }
        let move_type = *self.get_move_type(env)?;
        let date = date.unwrap_or_else(|| Utc::now().date_naive());
        let journal: Journal<SingleId> = self.get_journal(env)?;
        let partner: Contact<SingleId> = self.get_partner(env)?;
        let currency = self.currency_or_company(env)?;
        let mut values = MapOfFields::default();
        values.insert("move_type", move_type.reversed());
        values.insert("journal", journal.get_id());
        values.insert("date", date);
        values.insert("currency", currency.get_id());
        values.insert("reversed_entry", self.get_id());
        values.insert("reference", format!("Reversal of {}", self.get_name(env)?));
        if let Some(reason) = reason {
            values.insert("reversal_reason", reason);
        }
        if let Some(partner) = partner.get_optional_id() {
            values.insert("partner", partner);
        }
        if move_type.is_invoice() {
            values.insert("invoice_date", date);
            let term: PaymentTerm<SingleId> = self.get_payment_term(env)?;
            if let Some(term) = term.get_optional_id() {
                values.insert("payment_term", term);
            }
            let position: FiscalPosition<SingleId> = self.get_fiscal_position(env)?;
            if let Some(position) = position.get_optional_id() {
                values.insert("fiscal_position", position);
            }
            let mut copies = Vec::new();
            for line in &self.get_invoice_lines::<InvoiceLine<MultipleIds>>(env)? {
                let mut copy = MapOfFields::default();
                let product: product::models::Product<SingleId> = line.get_product(env)?;
                if let Some(product) = product.get_optional_id() {
                    copy.insert("product", product);
                }
                copy.insert_option("name", line.get_name(env)?.cloned());
                let account: Account<SingleId> = line.get_account(env)?;
                copy.insert("account", account.get_id());
                copy.insert("quantity", *line.get_quantity(env)?);
                let uom: uom::models::Uom<SingleId> = line.get_uom(env)?;
                if let Some(uom) = uom.get_optional_id() {
                    copy.insert("uom", uom);
                }
                copy.insert("price_unit", *line.get_price_unit(env)?);
                copy.insert("discount", *line.get_discount(env)?);
                let taxes: crate::models::tax::Tax<MultipleIds> = line.get_taxes(env)?;
                copy.insert("taxes", FieldType::Refs(taxes.get_ids_ref().clone()));
                copy.insert("sequence", *line.get_sequence(env)?);
                copies.push(copy);
            }
            values.insert_field_type(
                "invoice_lines",
                FieldType::Commands(vec![Command::Create(copies)]),
            );
        } else {
            let mut mirrored = Vec::new();
            for line in &self.get_lines::<MoveLine<MultipleIds>>(env)? {
                let mut copy = MapOfFields::default();
                let account: Account<SingleId> = line.get_account(env)?;
                copy.insert("account", account.get_id());
                let partner: Contact<SingleId> = line.get_partner(env)?;
                if let Some(partner) = partner.get_optional_id() {
                    copy.insert("partner", partner);
                }
                copy.insert_option("name", line.get_name(env)?.cloned());
                copy.insert("debit", *line.get_credit(env)?);
                copy.insert("credit", *line.get_debit(env)?);
                copy.insert("amount_currency", -*line.get_amount_currency(env)?);
                copy.insert("currency", currency.get_id());
                mirrored.push(copy);
            }
            values.insert_field_type(
                "lines",
                FieldType::Commands(vec![Command::Create(mirrored)]),
            );
        }
        let reversal: Move<SingleId> = env.create_new_record_from_map(values)?;
        Move::<MultipleIds>::from_ids(vec![self.get_id()], env)
            .link_reversal(env, reversal.get_id())?;
        if !move_type.is_invoice() {
            Move::<MultipleIds>::from_ids(vec![reversal.get_id()], env).action_post(env)?;
        }
        Ok(reversal)
    }
}

/// What tax items are grouped by: the tax, the account its share goes to, the grids, and the
/// account of the line it was computed on (where a share without an account of its own goes).
type TaxItemKey = (u32, Option<u32>, Vec<u32>, u32);

/// Put a signed amount on a journal item: positive as a debit, negative as a credit.
fn put_amounts(item: &mut MapOfFields, balance: Decimal, amount_currency: Decimal) {
    if balance >= Decimal::ZERO {
        item.insert("debit", balance);
        item.insert("credit", Decimal::ZERO);
    } else {
        item.insert("debit", Decimal::ZERO);
        item.insert("credit", -balance);
    }
    item.insert("amount_currency", amount_currency);
}

/// The fields of a posted entry that make what was recorded: they change only once it is back
/// to draft.
const RECORDED: [&str; 11] = [
    "move_type",
    "journal",
    "date",
    "invoice_date",
    "partner",
    "currency",
    "payment_term",
    "fiscal_position",
    "invoice_lines",
    "lines",
    "name",
];

#[erp_methods]
impl Move<MultipleIds> {
    /// The partner's terms on its side of the business: customer or supplier.
    pub fn compute_payment_term(&self, env: &mut Environment) -> Result<()> {
        for entry in self {
            let partner: Contact<SingleId> = entry.get_partner(env)?;
            let move_type = *entry.get_move_type(env)?;
            if partner.is_empty() || !move_type.is_invoice() {
                entry.set_payment_term(None::<&PaymentTerm<SingleId>>, env)?;
                continue;
            }
            let term: PaymentTerm<SingleId> = {
                let env = &mut *env.sudo();
                let partner: ContactAccount<SingleId> = env.get_record(partner.get_id().into());
                if move_type.is_sale() {
                    partner.get_customer_payment_term(env)?
                } else {
                    partner.get_supplier_payment_term(env)?
                }
            };
            entry.set_payment_term(&term, env)?;
        }
        Ok(())
    }

    /// The partner's fiscal position.
    pub fn compute_fiscal_position(&self, env: &mut Environment) -> Result<()> {
        for entry in self {
            let partner: Contact<SingleId> = entry.get_partner(env)?;
            if partner.is_empty() || !entry.get_move_type(env)?.is_invoice() {
                entry.set_fiscal_position(None::<&FiscalPosition<SingleId>>, env)?;
                continue;
            }
            let position: FiscalPosition<SingleId> = {
                let env = &mut *env.sudo();
                let partner: ContactAccount<SingleId> = env.get_record(partner.get_id().into());
                partner.get_fiscal_position(env)?
            };
            entry.set_fiscal_position(&position, env)?;
        }
        Ok(())
    }

    /// The last installment's date: from the items once posted, from the terms before.
    pub fn compute_invoice_date_due(&self, env: &mut Environment) -> Result<()> {
        for entry in self {
            let lines: MoveLine<MultipleIds> = entry.get_lines(env)?;
            let mut due: Option<NaiveDate> = None;
            for line in &lines {
                if matches!(*line.get_display_type(env)?, LineKind::PaymentTerm) {
                    due = due.max(line.get_date_maturity(env)?.copied());
                }
            }
            if due.is_none() && entry.get_move_type(env)?.is_invoice() {
                let date = entry
                    .get_invoice_date(env)?
                    .copied()
                    .unwrap_or(*entry.get_date(env)?);
                let term: PaymentTerm<SingleId> = entry.get_payment_term(env)?;
                let installments = term.compute(env, Decimal::ONE, date, Decimal::ZERO)?;
                due = installments.into_iter().map(|(date, _)| date).max();
            }
            entry.set_invoice_date_due(due, env)?;
        }
        Ok(())
    }

    /// An invoice adds up its lines; an entry made by hand, its debits.
    pub fn compute_amounts(&self, env: &mut Environment) -> Result<()> {
        for entry in self {
            if entry.get_move_type(env)?.is_invoice() {
                let lines: InvoiceLine<MultipleIds> = entry.get_invoice_lines(env)?;
                let untaxed: Decimal = lines.get_price_subtotal(env)?.into_iter().copied().sum();
                let total: Decimal = lines.get_price_total(env)?.into_iter().copied().sum();
                entry.set_amount_untaxed(untaxed, env)?;
                entry.set_amount_tax(total - untaxed, env)?;
                entry.set_amount_total(total, env)?;
            } else {
                let (debit, _) = entry.totals(env)?;
                entry.set_amount_untaxed(debit, env)?;
                entry.set_amount_tax(Decimal::ZERO, env)?;
                entry.set_amount_total(debit, env)?;
            }
        }
        Ok(())
    }

    /// What the partner still owes, or is owed, in the invoice's currency, and so whether it is
    /// paid; reversed when its own credit note settled it.
    pub fn compute_residual(&self, env: &mut Environment) -> Result<()> {
        for entry in self {
            let move_type = *entry.get_move_type(env)?;
            let posted = entry.is_posted(env)?;
            let lines: MoveLine<MultipleIds> = entry.get_lines(env)?;
            let mut residual = Decimal::ZERO;
            let mut reconcilable = false;
            for line in &lines {
                if matches!(*line.get_display_type(env)?, LineKind::PaymentTerm) {
                    reconcilable = true;
                    residual += *line.get_amount_residual_currency(env)?;
                }
            }
            let residual = residual.abs();
            let total = *entry.get_amount_total(env)?;
            let state = if !posted || !move_type.is_invoice() || !reconcilable {
                PaymentState::NotPaid
            } else if residual.is_zero() {
                if entry.settled_by_reversal(env)? {
                    PaymentState::Reversed
                } else {
                    PaymentState::Paid
                }
            } else if residual < total.abs() {
                PaymentState::Partial
            } else {
                PaymentState::NotPaid
            };
            entry.set_amount_residual(if posted { residual } else { total.abs() }, env)?;
            entry.set_payment_state(state, env)?;
        }
        Ok(())
    }

    /// The total in the company's currency, negative for credit notes and vendor bills, as
    /// lists of invoices add them up.
    pub fn compute_signed(&self, env: &mut Environment) -> Result<()> {
        for entry in self {
            let lines: MoveLine<MultipleIds> = entry.get_lines(env)?;
            let mut total = Decimal::ZERO;
            for line in &lines {
                if matches!(*line.get_display_type(env)?, LineKind::PaymentTerm) {
                    total += *line.get_balance(env)?;
                }
            }
            if !entry.get_move_type(env)?.is_invoice() {
                total = entry.totals(env)?.0;
            }
            entry.set_amount_total_signed(total, env)?;
        }
        Ok(())
    }

    /// What an entry is created with when nothing is said: today, the journal of its type, the
    /// company's currency.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let mut values = values;
        let today = Utc::now().date_naive();
        let company_currency = Currency::of_company(env)?;
        for entry in &mut values {
            let move_type = entry
                .get_option::<&String>("move_type")
                .map(|key| MoveType::from_key(key))
                .unwrap_or(MoveType::Entry);
            if entry.get_option::<&NaiveDate>("date").is_none() {
                let date = entry
                    .get_option::<&NaiveDate>("invoice_date")
                    .copied()
                    .unwrap_or(today);
                entry.insert("date", date);
            }
            if entry
                .get_option::<&u32>("journal")
                .is_none_or(|id| *id == 0)
            {
                let journal_type = match move_type {
                    MoveType::OutInvoice | MoveType::OutRefund => JournalType::Sale,
                    MoveType::InInvoice | MoveType::InRefund => JournalType::Purchase,
                    _ => JournalType::General,
                };
                let journal = Journal::first_of_type(env, journal_type)?;
                if let Some(journal) = journal.get_optional_id() {
                    entry.insert("journal", journal);
                }
            }
            if entry
                .get_option::<&u32>("currency")
                .is_none_or(|id| *id == 0)
                && let Some(currency) = company_currency.get_optional_id()
            {
                entry.insert("currency", currency);
            }
        }
        sup.call_with(values, env)
    }

    /// A posted entry keeps what it recorded; a cancelled one stays as it was.
    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        let recorded: Vec<&str> = values
            .fields
            .keys()
            .map(String::as_str)
            .filter(|field| RECORDED.contains(field))
            .collect();
        if !recorded.is_empty() {
            for entry in self {
                if !entry.is_draft(env)? {
                    let status = if entry.is_posted(env)? {
                        "posted"
                    } else {
                        "cancelled"
                    };
                    return Err(format!(
                        "{} is {status}: reset it to draft to change {}",
                        entry.get_name(env)?,
                        recorded.join(", ")
                    )
                    .into());
                }
            }
        }
        sup.call_with(values, env)
    }

    /// Only an entry that never got a number may be deleted: a numbered one is cancelled, so the
    /// numbering has no gap.
    pub fn delete(&self, env: &mut Environment, sup: Super) -> Result<u32> {
        for entry in self {
            if entry.is_posted(env)? || entry.get_name(env)? != "/" {
                return Err(format!(
                    "{} has been numbered: cancel it rather than delete it",
                    entry.get_name(env)?
                )
                .into());
            }
        }
        sup.call(env)
    }

    /// Post the entries: invoices get their journal items, every entry is checked to balance,
    /// numbered in its journal, and locked.
    #[erp(rpc)]
    pub fn action_post(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for entry in self {
                entry.post_one(env)?;
            }
            Ok(true)
        })
    }

    /// Bring posted or cancelled entries back to draft, so they can be changed: unless paid, or
    /// dated within locked books.
    #[erp(rpc)]
    pub fn button_draft(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for entry in self {
                let date = *entry.get_date(env)?;
                CompanyAccount::current(env)?.check_lock(env, date)?;
                let lines: MoveLine<MultipleIds> = entry.get_lines(env)?;
                for line in &lines {
                    let matched_debits: crate::models::reconcile::PartialReconcile<MultipleIds> =
                        line.get_matched_debits(env)?;
                    let matched_credits: crate::models::reconcile::PartialReconcile<MultipleIds> =
                        line.get_matched_credits(env)?;
                    if !matched_debits.get_ids_ref().is_empty()
                        || !matched_credits.get_ids_ref().is_empty()
                    {
                        return Err(format!(
                            "{} is matched with a payment: undo the matching first",
                            entry.get_name(env)?
                        )
                        .into());
                    }
                }
                entry.set_state(MoveState::Draft, &mut env.sudo())?;
                if entry.get_move_type(env)?.is_invoice() {
                    entry.clear_items(env)?;
                }
            }
            Ok(true)
        })
    }

    /// Cancel draft entries: kept, with their number if they had one, counting for nothing.
    #[erp(rpc)]
    pub fn button_cancel(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for entry in self {
                if !entry.is_draft(env)? {
                    return Err(format!(
                        "{} is posted: reset it to draft before cancelling it",
                        entry.get_name(env)?
                    )
                    .into());
                }
                entry.set_state(MoveState::Cancel, &mut env.sudo())?;
            }
            Ok(true)
        })
    }

    /// Make the credit note of each posted invoice, in draft, for all of it; an entry made by
    /// hand gets its reverse entry, posted. Opens what was made.
    #[erp(rpc)]
    pub fn action_reverse(&self, env: &mut Environment) -> Result<Value> {
        let reversals = env.savepoint(|env| {
            let mut made = Vec::new();
            for entry in self {
                made.push(entry.reverse_one(env, None, None)?);
            }
            Ok(made)
        })?;
        Ok(match reversals.as_slice() {
            [single] => {
                let action = single.action_xml_id(env)?;
                json!({"type": "open", "action": action, "id": single.get_id()})
            }
            [first, ..] => {
                let action = first.action_xml_id(env)?;
                let ids: Vec<u32> = reversals.iter().map(|entry| entry.get_id()).collect();
                json!({"type": "open", "action": action, "ids": ids})
            }
            [] => json!({"type": "reload"}),
        })
    }

    /// Prepare the payment of what is left to pay on the invoices of one partner: a draft
    /// payment, opened to be checked and confirmed.
    #[erp(rpc)]
    pub fn action_register_payment(&self, env: &mut Environment) -> Result<Value> {
        let payment = self.prepare_payment(env)?;
        let action = if matches!(*payment.get_partner_type(env)?, PartnerType::Customer) {
            "account.action_payments_received"
        } else {
            "account.action_payments_sent"
        };
        Ok(json!({"type": "open", "action": action, "id": payment.get_id()}))
    }

    /// Cancel posted invoices by a credit note settling them in full: posted, matched, and the
    /// invoice marked reversed.
    #[erp(rpc)]
    pub fn action_reverse_and_reconcile(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for entry in self {
                let reversal = entry.reverse_one(env, None, None)?;
                let reversal: Move<MultipleIds> = Move::from_ids(vec![reversal.get_id()], env);
                reversal.action_post(env)?;
                let mut ids = Vec::new();
                for line in &entry.get_lines::<MoveLine<MultipleIds>>(env)? {
                    if matches!(*line.get_display_type(env)?, LineKind::PaymentTerm) {
                        ids.push(line.get_id());
                    }
                }
                for line in reversal.get_lines::<MoveLine<MultipleIds>>(env)? {
                    if matches!(*line.get_display_type(env)?, LineKind::PaymentTerm) {
                        ids.push(line.get_id());
                    }
                }
                MoveLine::<MultipleIds>::from_ids(ids, env).reconcile_lines(env)?;
            }
            Ok(true)
        })
    }

    /// The payment reference of a posted invoice: its number. A localization may give it its
    /// own form, a Belgian structured communication.
    pub fn assign_payment_reference(&self, env: &mut Environment) -> Result<()> {
        for entry in self {
            if entry.get_payment_reference(env)?.is_none() {
                let name = entry.get_name(env)?.clone();
                entry.set_payment_reference(Some(name), env)?;
            }
        }
        Ok(())
    }

    /// Tie the credit note made from this invoice to what the invoice came from: nothing here;
    /// sales link its lines to the order lines the invoice's lines invoiced.
    pub fn link_reversal(&self, _env: &mut Environment, _reversal: u32) -> Result<()> {
        Ok(())
    }

    /// The draft payment of what is left on these posted invoices of one partner.
    pub fn prepare_payment(&self, env: &mut Environment) -> Result<Payment<SingleId>> {
        let mut partner = None;
        let mut total = Decimal::ZERO;
        let mut kind = None;
        let mut currency = None;
        let mut memo = Vec::new();
        for invoice in self {
            if !invoice.is_posted(env)? {
                return Err(format!(
                    "{} is not posted: it cannot be paid yet",
                    invoice.get_name(env)?
                )
                .into());
            }
            let move_type = *invoice.get_move_type(env)?;
            if !move_type.is_invoice() {
                return Err(format!("{} is no invoice", invoice.get_name(env)?).into());
            }
            let this_partner: Contact<SingleId> = invoice.get_partner(env)?;
            if partner.is_some_and(|partner| partner != this_partner.get_id()) {
                return Err("Invoices of different partners are paid separately".into());
            }
            partner = Some(this_partner.get_id());
            let this_currency = invoice.currency_or_company(env)?.get_id();
            if currency.is_some_and(|currency| currency != this_currency) {
                return Err("Invoices in different currencies are paid separately".into());
            }
            currency = Some(this_currency);
            let residual = *invoice.get_amount_residual(env)?;
            // A credit note pays back: it lowers what the invoices of the same side ask.
            let sign = if matches!(move_type, MoveType::OutRefund | MoveType::InRefund) {
                -Decimal::ONE
            } else {
                Decimal::ONE
            };
            total += sign * residual;
            kind = Some(move_type.is_sale());
            memo.push(
                invoice
                    .get_payment_reference(env)?
                    .cloned()
                    .unwrap_or(invoice.get_name(env)?.clone()),
            );
        }
        let (Some(partner), Some(sale)) = (partner, kind) else {
            return Err("Choose the invoices to pay".into());
        };
        if total.is_zero() {
            return Err("There is nothing left to pay".into());
        }
        let refund = total < Decimal::ZERO;
        let inbound = sale != refund;
        let mut values = MapOfFields::default();
        values.insert("partner", partner);
        values.insert("partner_type", if sale { "customer" } else { "supplier" });
        values.insert("payment_type", if inbound { "inbound" } else { "outbound" });
        values.insert("amount", total.abs());
        values.insert("memo", memo.join(", "));
        if let Some(currency) = currency {
            values.insert("currency", currency);
        }
        values.insert("invoices", FieldType::Refs(self.get_ids_ref().clone()));
        env.create_new_record_from_map(values)
    }
}

/// What the type of document is called in messages.
fn move_type_label(move_type: MoveType) -> &'static str {
    match move_type {
        MoveType::OutInvoice => "invoice",
        MoveType::OutRefund => "credit note",
        MoveType::InInvoice => "vendor bill",
        MoveType::InRefund => "vendor credit note",
        _ => "entry",
    }
}
