use crate::models::account::{Account, BaseAccount};
use crate::models::moves::{BaseAccountMove, Move, MoveState};
use crate::models::reconcile::{
    BaseAccountFullReconcile, BaseAccountPartialReconcile, PartialReconcile,
};
use crate::models::tax::{BaseAccountTax, BaseAccountTaxTag};
use base::models::BaseContact;
use code_gen::{Model, erp_methods, selection};
use currency::models::BaseCurrency;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId};
use erp::types::model::MapOfFields;
use product::models::BaseProduct;

#[selection]
pub enum LineKind {
    #[default]
    #[selection(label = "Entry")]
    Entry,
    #[selection(label = "Product")]
    Product,
    #[selection(label = "Tax")]
    Tax,
    #[selection(label = "Payment term")]
    PaymentTerm,
}

/// A journal item: an amount debited or credited to an account, for a partner, as part of an
/// entry — what the ledgers and the tax return are made of.
#[derive(Model)]
#[erp(id = "account_move_line", methods)]
#[allow(dead_code)]
pub struct MoveLine<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Entry", required, ondelete = "cascade")]
    move_id: Reference<BaseAccountMove, SingleId>,
    #[erp(required, ondelete = "restrict")]
    account: Reference<BaseAccount, SingleId>,
    #[erp(ondelete = "restrict")]
    partner: Reference<BaseContact, SingleId>,
    #[erp(label = "Label")]
    name: Option<String>,
    #[erp(default = 0.0)]
    debit: Decimal,
    #[erp(default = 0.0)]
    credit: Decimal,
    #[erp(compute = "compute_balance", depends = ["debit", "credit"], stored)]
    balance: Decimal,
    #[erp(
        label = "Amount in currency",
        default = 0.0,
        description = "The signed amount in the entry's currency"
    )]
    amount_currency: Decimal,
    #[erp(ondelete = "restrict")]
    currency: Reference<BaseCurrency, SingleId>,
    #[erp(compute = "compute_date", depends = ["move_id.date"], stored, index)]
    date: Option<NaiveDate>,
    #[erp(label = "Due date", index)]
    date_maturity: Option<NaiveDate>,
    #[erp(label = "Status", compute = "compute_parent_state", depends = ["move_id.state"], stored, index)]
    parent_state: MoveState,
    #[erp(label = "Kind")]
    display_type: LineKind,
    #[erp(label = "Originating tax", ondelete = "restrict")]
    tax_line: Reference<BaseAccountTax, SingleId>,
    #[erp(label = "Base of the tax", default = 0.0)]
    tax_base_amount: Decimal,
    #[erp(label = "Taxes", relation = "account_move_line_tax_rel")]
    taxes: Reference<BaseAccountTax, MultipleIds>,
    #[erp(label = "Tax grids", relation = "account_move_line_tag_rel")]
    tax_tags: Reference<BaseAccountTaxTag, MultipleIds>,
    #[erp(ondelete = "restrict")]
    product: Reference<BaseProduct, SingleId>,
    #[erp(default = 0.0)]
    quantity: Decimal,
    #[erp(label = "Open amount", default = 0.0)]
    amount_residual: Decimal,
    #[erp(label = "Open amount in currency", default = 0.0)]
    amount_residual_currency: Decimal,
    #[erp(index)]
    reconciled: bool,
    #[erp(label = "Matching", ondelete = "set_null")]
    full_reconcile: Reference<BaseAccountFullReconcile, SingleId>,
    #[erp(label = "Matched debits", inverse = "credit_line")]
    matched_debits: Reference<BaseAccountPartialReconcile, MultipleIds>,
    #[erp(label = "Matched credits", inverse = "debit_line")]
    matched_credits: Reference<BaseAccountPartialReconcile, MultipleIds>,
}

impl MoveLine<SingleId> {
    /// Whether the line is left open until matched: on a reconciled account.
    pub fn is_reconcilable(&self, env: &mut Environment) -> Result<bool> {
        let env = &mut *env.sudo();
        let account: Account<SingleId> = self.get_account(env)?;
        Ok(*account.get_reconcile(env)?)
    }

    /// Whether the line's entry is in another currency than the company's.
    pub fn in_foreign_currency(&self, env: &mut Environment) -> Result<bool> {
        let currency: currency::models::Currency<SingleId> = self.get_currency(env)?;
        let company = currency::models::Currency::of_company(env)?;
        Ok(!currency.is_empty() && currency.get_id() != company.get_id())
    }
}

#[erp_methods]
impl MoveLine<MultipleIds> {
    pub fn compute_balance(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let balance = *line.get_debit(env)? - *line.get_credit(env)?;
            line.set_balance(balance, env)?;
        }
        Ok(())
    }

    pub fn compute_date(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let entry: Move<SingleId> = line.get_move_id(env)?;
            let date = if entry.is_empty() {
                None
            } else {
                Some(*entry.get_date(env)?)
            };
            line.set_date(date, env)?;
        }
        Ok(())
    }

    pub fn compute_parent_state(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let entry: Move<SingleId> = line.get_move_id(env)?;
            let state = if entry.is_empty() {
                MoveState::Draft
            } else {
                *entry.get_state(env)?
            };
            line.set_parent_state(state, env)?;
        }
        Ok(())
    }

    /// A line is a debit or a credit, never both, never negative; what is left open of it starts
    /// as all of it.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let mut values = values;
        for line in &mut values {
            let debit = line
                .get_option::<&Decimal>("debit")
                .copied()
                .unwrap_or_default();
            let credit = line
                .get_option::<&Decimal>("credit")
                .copied()
                .unwrap_or_default();
            if debit < Decimal::ZERO || credit < Decimal::ZERO {
                return Err("A journal item's debit and credit are not negative".into());
            }
            if !debit.is_zero() && !credit.is_zero() {
                return Err("A journal item is a debit or a credit, not both".into());
            }
            line.insert("amount_residual", debit - credit);
            if !line.contains_key("amount_currency") {
                line.insert("amount_currency", debit - credit);
            }
            let currency = line
                .get_option::<&Decimal>("amount_currency")
                .copied()
                .unwrap_or_default();
            line.insert("amount_residual_currency", currency);
        }
        sup.call_with(values, env)
    }

    /// The items of a posted entry are what was recorded: only their matching moves on.
    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        const MATCHING: [&str; 4] = [
            "amount_residual",
            "amount_residual_currency",
            "reconciled",
            "full_reconcile",
        ];
        let touches_amounts = values.fields.keys().any(|field| {
            !MATCHING.contains(&field.as_str()) && field != "date_maturity" && field != "name"
        });
        if touches_amounts {
            for line in self {
                let entry: Move<SingleId> = line.get_move_id(env)?;
                if !entry.is_empty() && !entry.is_draft(env)? {
                    return Err(format!(
                        "{} is posted: its journal items cannot change",
                        entry.get_name(env)?
                    )
                    .into());
                }
            }
        }
        sup.call_with(values, env)
    }

    /// Settle these journal items against each other.
    #[erp(rpc)]
    pub fn reconcile(&self, env: &mut Environment) -> Result<bool> {
        self.reconcile_lines(env)?;
        Ok(true)
    }

    /// Undo the matchings of these items, and of the items matched with them.
    #[erp(rpc)]
    pub fn remove_move_reconcile(&self, env: &mut Environment) -> Result<bool> {
        self.unreconcile_lines(env)?;
        Ok(true)
    }

    /// What is left open of each line: its balance less what settled it, in both currencies;
    /// reconciled once nothing is.
    pub fn refresh_residuals(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        for line in self {
            let mut residual = *line.get_balance(env)?;
            let mut residual_currency = *line.get_amount_currency(env)?;
            let as_debit: PartialReconcile<MultipleIds> = line.get_matched_credits(env)?;
            for partial in &as_debit {
                residual -= *partial.get_amount(env)?;
                residual_currency -= *partial.get_debit_amount_currency(env)?;
            }
            let as_credit: PartialReconcile<MultipleIds> = line.get_matched_debits(env)?;
            for partial in &as_credit {
                residual += *partial.get_amount(env)?;
                residual_currency += *partial.get_credit_amount_currency(env)?;
            }
            let foreign = line.in_foreign_currency(env)?;
            let reconciled = line.is_reconcilable(env)?
                && residual.is_zero()
                && (!foreign || residual_currency.is_zero());
            line.set_amount_residual(residual, env)?;
            line.set_amount_residual_currency(residual_currency, env)?;
            line.set_reconciled(reconciled, env)?;
        }
        Ok(())
    }
}
