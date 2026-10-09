use crate::models::account::Account;
use crate::models::company::CompanyAccount;
use crate::models::contact::ContactAccount;
use crate::models::journal::{BaseAccountJournal, Journal, JournalType};
use crate::models::move_line::{LineKind, MoveLine};
use crate::models::moves::{BaseAccountMove, Move};
use base::models::{BaseContact, Contact};
use code_gen::{Model, erp_methods, selection};
use currency::models::{BaseCurrency, Currency};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{
    Command, Decimal, FieldType, IdMode, MultipleIds, NaiveDate, Reference, SingleId, Utc,
};
use erp::types::model::MapOfFields;

#[selection]
pub enum PaymentType {
    #[default]
    #[selection(label = "Receive money")]
    Inbound,
    #[selection(label = "Send money")]
    Outbound,
}

#[selection]
pub enum PartnerType {
    #[default]
    #[selection(label = "Customer")]
    Customer,
    #[selection(label = "Vendor")]
    Supplier,
}

#[selection]
pub enum PaymentStatus {
    #[default]
    #[selection(label = "Draft")]
    Draft,
    #[selection(label = "Posted")]
    Posted,
    #[selection(label = "Cancelled")]
    Cancel,
}

/// Money received from a customer or sent to a vendor, through a bank or cash journal, and the
/// invoices it pays.
#[derive(Model)]
#[erp(id = "account_payment", order = "date desc, id desc", methods)]
#[allow(dead_code)]
pub struct Payment<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Number", default = "/", index = "trigram")]
    name: String,
    #[erp(label = "Payment type", tracking)]
    payment_type: PaymentType,
    #[erp(label = "Partner type")]
    partner_type: PartnerType,
    #[erp(required, ondelete = "restrict", tracking)]
    partner: Reference<BaseContact, SingleId>,
    #[erp(default = 0.0, tracking)]
    amount: Decimal,
    #[erp(ondelete = "restrict")]
    currency: Reference<BaseCurrency, SingleId>,
    #[erp(tracking, index)]
    date: NaiveDate,
    #[erp(
        required,
        ondelete = "restrict",
        domain = r#"[["journal_type", "in", ["bank", "cash"]]]"#
    )]
    journal: Reference<BaseAccountJournal, SingleId>,
    #[erp(label = "Memo")]
    memo: Option<String>,
    #[erp(label = "Status", tracking, index)]
    state: PaymentStatus,
    #[erp(label = "Journal entry", ondelete = "restrict")]
    move_id: Reference<BaseAccountMove, SingleId>,
    #[erp(label = "Invoices paid", relation = "account_payment_invoice_rel")]
    invoices: Reference<BaseAccountMove, MultipleIds>,
}

#[erp_methods]
impl Payment<SingleId> {
    /// The account the payment settles: the partner's receivable or payable.
    fn counterpart_account(&self, env: &mut Environment) -> Result<Account<SingleId>> {
        let customer = matches!(*self.get_partner_type(env)?, PartnerType::Customer);
        let partner: Contact<SingleId> = self.get_partner(env)?;
        let env = &mut *env.sudo();
        let partner: ContactAccount<SingleId> = env.get_record(partner.get_id().into());
        let account: Account<SingleId> = if customer {
            partner.get_account_receivable(env)?
        } else {
            partner.get_account_payable(env)?
        };
        if !account.is_empty() {
            return Ok(account);
        }
        CompanyAccount::current(env)?.required_account(
            env,
            if customer {
                "receivable".to_string()
            } else {
                "payable".to_string()
            },
        )
    }

    /// Where the money waits for the bank statement: the journal's outstanding account, else
    /// the bank account itself.
    fn liquidity_account(&self, env: &mut Environment) -> Result<Account<SingleId>> {
        let inbound = matches!(*self.get_payment_type(env)?, PaymentType::Inbound);
        let journal: Journal<SingleId> = self.get_journal(env)?;
        let env = &mut *env.sudo();
        let outstanding: Account<SingleId> = if inbound {
            journal.get_outstanding_receipt_account(env)?
        } else {
            journal.get_outstanding_payment_account(env)?
        };
        if !outstanding.is_empty() {
            return Ok(outstanding);
        }
        let account: Account<SingleId> = journal.get_default_account(env)?;
        if account.is_empty() {
            return Err(
                format!("The journal {} has no bank account", journal.get_name(env)?).into(),
            );
        }
        Ok(account)
    }

    fn post_one(&self, env: &mut Environment) -> Result<()> {
        if !matches!(*self.get_state(env)?, PaymentStatus::Draft) {
            return Err(format!(
                "{} is not a draft: it cannot be confirmed",
                self.get_name(env)?
            )
            .into());
        }
        let amount = *self.get_amount(env)?;
        if amount <= Decimal::ZERO {
            return Err("A payment's amount must be positive".into());
        }
        let inbound = matches!(*self.get_payment_type(env)?, PaymentType::Inbound);
        let date = *self.get_date(env)?;
        let currency: Currency<SingleId> = self.get_currency(env)?;
        let company_currency = Currency::of_company(env)?;
        let currency = if currency.is_empty() {
            company_currency.clone()
        } else {
            currency
        };
        let balance = currency.convert(env, amount, company_currency, date)?;
        let partner: Contact<SingleId> = self.get_partner(env)?;
        let journal: Journal<SingleId> = self.get_journal(env)?;
        let liquidity = self.liquidity_account(env)?;
        let counterpart = self.counterpart_account(env)?;
        let memo = self.get_memo(env)?.cloned().unwrap_or_default();
        let sign = if inbound { Decimal::ONE } else { -Decimal::ONE };
        let item = |account: &Account<SingleId>,
                    signed: Decimal,
                    signed_currency: Decimal,
                    kind: LineKind| {
            let mut item = MapOfFields::default();
            item.insert("account", account.get_id());
            item.insert("partner", partner.get_id());
            item.insert("name", memo.clone());
            item.insert("currency", currency.get_id());
            item.insert("amount_currency", signed_currency);
            item.insert("display_type", kind);
            if signed >= Decimal::ZERO {
                item.insert("debit", signed);
            } else {
                item.insert("credit", -signed);
            }
            item
        };
        let items = vec![
            item(&liquidity, sign * balance, sign * amount, LineKind::Entry),
            item(
                &counterpart,
                -sign * balance,
                -sign * amount,
                LineKind::PaymentTerm,
            ),
        ];
        let mut entry = MapOfFields::default();
        entry.insert("journal", journal.get_id());
        entry.insert("date", date);
        entry.insert("partner", partner.get_id());
        entry.insert("currency", currency.get_id());
        entry.insert("reference", memo.clone());
        entry.insert_field_type("lines", FieldType::Commands(vec![Command::Create(items)]));
        let entry: Move<MultipleIds> = env.sudo().create_new_records_from_maps(vec![entry])?;
        entry.action_post(&mut env.sudo())?;
        let entry: Move<SingleId> = env.get_record(entry.get_ids_ref()[0].into());
        let name = entry.get_name(&mut env.sudo())?.clone();
        self.set_move_id(&entry, env)?;
        self.set_state(PaymentStatus::Posted, env)?;
        self.set_name(name, env)?;

        let invoices: Move<MultipleIds> = self.get_invoices(env)?;
        if !invoices.get_ids_ref().is_empty() {
            let mut lines = Vec::new();
            let mut entries: Vec<Move<SingleId>> = invoices.into_iter().collect();
            entries.push(entry);
            {
                let sudo = &mut *env.sudo();
                for invoice in entries {
                    let items: MoveLine<MultipleIds> = invoice.get_lines(sudo)?;
                    for line in &items {
                        let account: Account<SingleId> = line.get_account(sudo)?;
                        if account.get_id() == counterpart.get_id()
                            && !*line.get_reconciled(sudo)?
                        {
                            lines.push(line.get_id());
                        }
                    }
                }
            }
            MoveLine::<MultipleIds>::from_ids(lines, env).reconcile_lines(env)?;
        }
        Ok(())
    }
}

#[erp_methods]
impl Payment<MultipleIds> {
    /// A payment is dated today, in the company's currency, through the first bank journal,
    /// unless said otherwise; a customer pays in, a vendor is paid.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let mut values = values;
        let today = Utc::now().date_naive();
        let company_currency = Currency::of_company(env)?;
        for payment in &mut values {
            if payment.get_option::<&NaiveDate>("date").is_none() {
                payment.insert("date", today);
            }
            if payment
                .get_option::<&u32>("journal")
                .is_none_or(|id| *id == 0)
            {
                let bank = Journal::first_of_type(env, JournalType::Bank)?;
                if let Some(bank) = bank.get_optional_id() {
                    payment.insert("journal", bank);
                }
            }
            if payment
                .get_option::<&u32>("currency")
                .is_none_or(|id| *id == 0)
                && let Some(currency) = company_currency.get_optional_id()
            {
                payment.insert("currency", currency);
            }
            if !payment.contains_key("payment_type") {
                let supplier = payment
                    .get_option::<&String>("partner_type")
                    .is_some_and(|kind| kind == "supplier");
                payment.insert(
                    "payment_type",
                    if supplier { "outbound" } else { "inbound" },
                );
            }
        }
        sup.call_with(values, env)
    }

    /// A confirmed payment keeps what it records: back to draft only through cancelling.
    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        const RECORDED: [&str; 7] = [
            "amount",
            "partner",
            "date",
            "journal",
            "currency",
            "payment_type",
            "partner_type",
        ];
        if values
            .fields
            .keys()
            .any(|field| RECORDED.contains(&field.as_str()))
        {
            for payment in self {
                if !matches!(*payment.get_state(env)?, PaymentStatus::Draft) {
                    return Err(format!(
                        "{} is confirmed: cancel it to change it",
                        payment.get_name(env)?
                    )
                    .into());
                }
            }
        }
        sup.call_with(values, env)
    }

    /// Confirm the payments: each records its entry, and settles the invoices it pays.
    #[erp(rpc)]
    pub fn action_post(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for payment in self {
                payment.post_one(env)?;
            }
            Ok(true)
        })
    }

    /// Cancel the payments: their entries are unmatched and cancelled, the invoices open again.
    #[erp(rpc)]
    pub fn action_cancel(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for payment in self {
                let entry: Move<SingleId> = payment.get_move_id(env)?;
                if !entry.is_empty() {
                    let lines: MoveLine<MultipleIds> = entry.get_lines(&mut env.sudo())?;
                    lines.unreconcile_lines(env)?;
                    let entry: Move<MultipleIds> = Move::from_ids(vec![entry.get_id()], env);
                    entry.button_draft(&mut env.sudo())?;
                    entry.button_cancel(&mut env.sudo())?;
                }
                payment.set_state(PaymentStatus::Cancel, env)?;
            }
            Ok(true)
        })
    }

    /// Bring cancelled payments back to draft, to correct and confirm them again.
    #[erp(rpc)]
    pub fn action_draft(&self, env: &mut Environment) -> Result<bool> {
        for payment in self {
            if matches!(*payment.get_state(env)?, PaymentStatus::Posted) {
                return Err(
                    format!("{} is confirmed: cancel it first", payment.get_name(env)?).into(),
                );
            }
            payment.set_state(PaymentStatus::Draft, env)?;
            payment.set_move_id(None::<&Move<SingleId>>, env)?;
        }
        Ok(true)
    }
}
