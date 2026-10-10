use crate::models::account::BaseAccount;
use code_gen::{Model, erp_methods, selection};
use currency::models::BaseCurrency;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::Selection;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use sequence::models::{BaseSequence, Sequence};

#[selection]
pub enum JournalType {
    #[selection(label = "Sales")]
    Sale,
    #[selection(label = "Purchases")]
    Purchase,
    #[selection(label = "Bank")]
    Bank,
    #[selection(label = "Cash")]
    Cash,
    #[default]
    #[selection(label = "Miscellaneous")]
    General,
}

/// A book entries are kept in: customer invoices, vendor bills, a bank account, or
/// miscellaneous operations — each numbering its entries in its own series.
#[derive(Model)]
#[erp(id = "account_journal", order = "sequence, id", methods)]
#[allow(dead_code)]
pub struct Journal<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(
        label = "Short code",
        description = "Starts the numbers of its entries, e.g. INV"
    )]
    code: String,
    #[erp(label = "Type")]
    journal_type: JournalType,
    #[erp(
        label = "Default account",
        ondelete = "restrict",
        description = "Income for sales, expenses for purchases, the bank account for a bank"
    )]
    default_account: Reference<BaseAccount, SingleId>,
    #[erp(
        label = "Outstanding receipts",
        ondelete = "restrict",
        description = "Where payments received wait for the bank statement showing them"
    )]
    outstanding_receipt_account: Reference<BaseAccount, SingleId>,
    #[erp(
        label = "Outstanding payments",
        ondelete = "restrict",
        description = "Where payments made wait for the bank statement showing them"
    )]
    outstanding_payment_account: Reference<BaseAccount, SingleId>,
    #[erp(ondelete = "restrict", description = "Left empty, the company's")]
    currency: Reference<BaseCurrency, SingleId>,
    #[erp(label = "Numbering", ondelete = "restrict")]
    sequence: Reference<BaseSequence, SingleId>,
    #[erp(label = "Credit note numbering", ondelete = "restrict")]
    refund_sequence: Reference<BaseSequence, SingleId>,
    #[erp(default = 10)]
    sequence_order: i32,
    #[erp(default = true)]
    active: bool,
}

#[erp_methods]
impl Journal<SingleId> {
    /// The first active journal of a type, empty when there is none.
    pub fn first_of_type(
        env: &mut Environment,
        journal_type: JournalType,
    ) -> Result<Journal<SingleId>> {
        let env = &mut *env.sudo();
        let found: Journal<MultipleIds> = env.search_with(
            &make_domain!([
                ("journal_type", "=", journal_type.key().as_str()),
                ("active", "=", true)
            ]),
            &erp_search::SearchOptions::new()
                .order_by(erp_search::OrderBy::asc("sequence_order"))
                .order_by(erp_search::OrderBy::asc("id")),
        )?;
        Ok(found
            .into_iter()
            .next()
            .unwrap_or_else(|| env.get_record(SingleId::empty())))
    }

    /// The series numbering this journal's entries, or its credit notes.
    pub fn numbering(&self, env: &mut Environment, refund: bool) -> Result<Sequence<SingleId>> {
        let env = &mut *env.sudo();
        if refund {
            let refunds: Sequence<SingleId> = self.get_refund_sequence(env)?;
            if !refunds.is_empty() {
                return Ok(refunds);
            }
        }
        self.get_sequence(env)
    }

    /// A code is unique among journals and short enough to start a number.
    fn check_journal(&self, env: &mut Environment) -> Result<()> {
        let code = self.get_code(env)?.trim().to_string();
        if code.is_empty() || code.len() > 8 {
            return Err(format!("A journal code has 1 to 8 characters, not \"{code}\"").into());
        }
        let same = env.sudo().count(
            "account_journal",
            &make_domain!([("code", "=", code.clone())]),
        )?;
        if same > 1 {
            return Err(format!("The journal code {code} is already used").into());
        }
        Ok(())
    }
}

#[erp_methods]
impl Journal<MultipleIds> {
    /// A journal gets its numbering when created without one: `CODE/{year}/00001`, and
    /// `RCODE/{year}/00001` for the credit notes of sales and purchases.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        env.savepoint(|env| {
            let ids: MultipleIds = sup.call_with(values, env)?;
            for journal in Journal::<MultipleIds>::from_ids(ids.clone(), env) {
                let code = journal.get_code(env)?.trim().to_uppercase();
                let name = journal.get_name(env)?.clone();
                let numbering: Sequence<SingleId> = journal.get_sequence(env)?;
                if numbering.is_empty() {
                    let id = new_numbering(
                        env,
                        &name,
                        &format!("{code}/{{year}}/"),
                        &format!("journal.{}", journal.get_id()),
                    )?;
                    journal.set_sequence(Reference::<BaseSequence, SingleId>::from(id), env)?;
                }
                let refunds: Sequence<SingleId> = journal.get_refund_sequence(env)?;
                if refunds.is_empty()
                    && matches!(
                        *journal.get_journal_type(env)?,
                        JournalType::Sale | JournalType::Purchase
                    )
                {
                    let id = new_numbering(
                        env,
                        &format!("{name} (credit notes)"),
                        &format!("R{code}/{{year}}/"),
                        &format!("journal.{}.refund", journal.get_id()),
                    )?;
                    journal
                        .set_refund_sequence(Reference::<BaseSequence, SingleId>::from(id), env)?;
                }
            }
            Ok(ids)
        })
    }
}

/// A yearly series for a journal, created as sudo: setting up a journal sets up its numbering.
fn new_numbering(env: &mut Environment, name: &str, prefix: &str, code: &str) -> Result<u32> {
    let env = &mut *env.sudo();
    let mut values = MapOfFields::default();
    values.insert("name", name);
    values.insert("code", code);
    values.insert("prefix", prefix);
    values.insert("padding", 5);
    values.insert("reset", "yearly");
    Ok(env.create_records("sequence", vec![values])?.get_ids_ref()[0])
}
