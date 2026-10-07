use crate::models::account::{Account, AccountType};
use crate::models::journal::{BaseAccountJournal, Journal};
use crate::models::move_line::{BaseAccountMoveLine, MoveLine};
use crate::models::moves::{BaseAccountMove, Move, MoveState};
use base::models::{BaseContact, Contact};
use code_gen::{Model, erp_methods, selection};
use currency::models::Currency;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::Selection;
use erp::types::field::{
    Command, Decimal, FieldType, IdMode, MultipleIds, NaiveDate, Reference, SingleId,
};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;

#[selection]
pub enum StatementState {
    #[default]
    #[selection(label = "Open")]
    Open,
    #[selection(label = "Validated")]
    Confirm,
}

/// A statement of a bank account: its lines, between a starting and an ending balance.
#[derive(Model)]
#[erp(id = "account_bank_statement", methods)]
#[allow(dead_code)]
pub struct BankStatement<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Reference", index = "trigram")]
    name: String,
    #[erp(
        required,
        ondelete = "restrict",
        domain = r#"[["journal_type", "=", "bank"]]"#
    )]
    journal: Reference<BaseAccountJournal, SingleId>,
    #[erp(index)]
    date: NaiveDate,
    #[erp(label = "Starting balance", default = 0.0)]
    balance_start: Decimal,
    #[erp(label = "Ending balance", default = 0.0)]
    balance_end_real: Decimal,
    #[erp(
        label = "Computed balance",
        compute = "compute_balance_end",
        depends = ["balance_start", "lines.amount"],
        stored
    )]
    balance_end: Decimal,
    #[erp(label = "Transactions", inverse = "statement", owned)]
    lines: Reference<BaseAccountBankStatementLine, MultipleIds>,
    #[erp(label = "Status", tracking)]
    state: StatementState,
}

/// A transaction on a bank statement, until matched with what it pays or is paid by.
#[derive(Model)]
#[erp(
    id = "account_bank_statement_line",
    name_field = "payment_ref",
    methods
)]
#[allow(dead_code)]
pub struct BankStatementLine<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    statement: Reference<BaseAccountBankStatement, SingleId>,
    date: NaiveDate,
    #[erp(label = "Label")]
    payment_ref: String,
    #[erp(ondelete = "restrict")]
    partner: Reference<BaseContact, SingleId>,
    #[erp(default = 0.0, description = "Positive when money came in")]
    amount: Decimal,
    #[erp(label = "Journal entry", ondelete = "restrict")]
    move_id: Reference<BaseAccountMove, SingleId>,
    #[erp(label = "Matched")]
    is_reconciled: bool,
    #[erp(
        label = "Items to match",
        relation = "account_bank_statement_line_match_rel",
        domain = r#"[["reconciled", "=", false], ["parent_state", "=", "posted"]]"#,
        description = "Open items this transaction settles, when it cannot find them itself"
    )]
    to_match: Reference<BaseAccountMoveLine, MultipleIds>,
}

#[erp_methods]
impl BankStatement<MultipleIds> {
    pub fn compute_balance_end(&self, env: &mut Environment) -> Result<()> {
        for statement in self {
            let lines: BankStatementLine<MultipleIds> = statement.get_lines(env)?;
            let moved: Decimal = lines.get_amount(env)?.into_iter().copied().sum();
            let end = *statement.get_balance_start(env)? + moved;
            statement.set_balance_end(end, env)?;
        }
        Ok(())
    }

    /// Match every transaction that finds what it pays, then validate the statement once all
    /// are matched and its balances agree.
    #[erp(rpc)]
    pub fn action_validate(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for statement in self {
                let lines: BankStatementLine<MultipleIds> = statement.get_lines(env)?;
                for line in &lines {
                    if !*line.get_is_reconciled(env)? {
                        line.reconcile_one(env)?;
                    }
                }
                let computed = *statement.get_balance_end(env)?;
                let real = *statement.get_balance_end_real(env)?;
                if computed != real {
                    return Err(format!(
                        "The statement ends at {real}, its transactions at {computed}"
                    )
                    .into());
                }
                statement.set_state(StatementState::Confirm, env)?;
            }
            Ok(true)
        })
    }
}

#[erp_methods]
impl BankStatementLine<MultipleIds> {
    /// Match the transactions with what they pay: the items chosen, else those found.
    #[erp(rpc)]
    pub fn action_reconcile(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for line in self {
                line.reconcile_one(env)?;
            }
            Ok(true)
        })
    }

    /// Undo the matching of the transactions: their entries are cancelled, what they paid is
    /// open again.
    #[erp(rpc)]
    pub fn action_undo_reconciliation(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for line in self {
                let entry: Move<SingleId> = line.get_move_id(env)?;
                if entry.is_empty() {
                    continue;
                }
                let items: MoveLine<MultipleIds> = entry.get_lines(&mut env.sudo())?;
                items.unreconcile_lines(env)?;
                let entry: Move<MultipleIds> = Move::from_ids(vec![entry.get_id()], env);
                entry.button_draft(&mut env.sudo())?;
                entry.button_cancel(&mut env.sudo())?;
                line.set_move_id(None::<&Move<SingleId>>, env)?;
                line.set_is_reconciled(false, env)?;
            }
            Ok(true)
        })
    }
}

#[erp_methods]
impl BankStatementLine<SingleId> {
    /// The open items this transaction settles, found by their payment reference, else by
    /// partner and amount: receivables, payables, and payments waiting for the bank.
    pub fn find_matches(&self, env: &mut Environment) -> Result<Vec<u32>> {
        let env = &mut *env.sudo();
        let amount = *self.get_amount(env)?;
        let label = self.get_payment_ref(env)?.trim().to_string();
        let partner: Contact<SingleId> = self.get_partner(env)?;
        let statement: BankStatement<SingleId> = self.get_statement(env)?;
        let journal: Journal<SingleId> = statement.get_journal(env)?;
        let mut accounts: Vec<u32> = Vec::new();
        for account in [
            journal.get_outstanding_receipt_account::<Account<SingleId>>(env)?,
            journal.get_outstanding_payment_account::<Account<SingleId>>(env)?,
        ] {
            if let Some(id) = account.get_optional_id() {
                accounts.push(id);
            }
        }
        let receivables: Account<MultipleIds> = env.search(&make_domain!([
            "|",
            (
                "account_type",
                "=",
                AccountType::AssetReceivable.key().as_str()
            ),
            (
                "account_type",
                "=",
                AccountType::LiabilityPayable.key().as_str()
            )
        ]))?;
        accounts.extend(receivables.get_ids_ref().iter().copied());
        let open: MoveLine<MultipleIds> = env.search(&make_domain!([
            ("account", "in", accounts),
            ("reconciled", "=", false),
            ("parent_state", "=", MoveState::Posted.key().as_str())
        ]))?;
        // Money in settles what is owed to the company — debits — and money out the reverse.
        let mut by_reference = Vec::new();
        let mut by_partner = Vec::new();
        for item in &open {
            let residual = *item.get_amount_residual(env)?;
            if residual.is_zero() || (residual > Decimal::ZERO) != (amount > Decimal::ZERO) {
                continue;
            }
            let entry: Move<SingleId> = item.get_move_id(env)?;
            let reference = entry
                .get_payment_reference(env)?
                .cloned()
                .unwrap_or_default();
            let normalize = |text: &str| -> String {
                text.chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_uppercase()
            };
            if !label.is_empty()
                && (normalize(&reference) == normalize(&label)
                    || normalize(entry.get_name(env)?) == normalize(&label))
            {
                by_reference.push(item.get_id());
                continue;
            }
            let item_partner: Contact<SingleId> = item.get_partner(env)?;
            if !partner.is_empty()
                && item_partner.get_id() == partner.get_id()
                && residual == amount
            {
                by_partner.push(item.get_id());
            }
        }
        if !by_reference.is_empty() {
            return Ok(by_reference);
        }
        Ok(by_partner.into_iter().take(1).collect())
    }

    /// Record the transaction in the bank journal against what it settles, and match them.
    fn reconcile_one(&self, env: &mut Environment) -> Result<()> {
        if *self.get_is_reconciled(env)? {
            return Ok(());
        }
        let chosen: MoveLine<MultipleIds> = self.get_to_match(env)?;
        let candidates = if chosen.get_ids_ref().is_empty() {
            self.find_matches(env)?
        } else {
            chosen.get_ids_ref().clone()
        };
        let label = self.get_payment_ref(env)?.clone();
        if candidates.is_empty() {
            return Err(
                format!("Nothing open matches \"{label}\": choose the items it settles").into(),
            );
        }
        let env = &mut *env.sudo();
        let amount = *self.get_amount(env)?;
        let date = *self.get_date(env)?;
        let statement: BankStatement<SingleId> = self.get_statement(env)?;
        let journal: Journal<SingleId> = statement.get_journal(env)?;
        let bank: Account<SingleId> = journal.get_default_account(env)?;
        if bank.is_empty() {
            return Err(
                format!("The journal {} has no bank account", journal.get_name(env)?).into(),
            );
        }
        let currency = Currency::of_company(env)?;
        let mut partner: Contact<SingleId> = self.get_partner(env)?;
        let mut left = amount;
        let mut items = Vec::new();
        let mut settled = Vec::new();
        for id in &candidates {
            let item: MoveLine<SingleId> = env.get_record((*id).into());
            let residual = *item.get_amount_residual(env)?;
            let take = if amount > Decimal::ZERO {
                residual.min(left)
            } else {
                residual.max(left)
            };
            if take.is_zero() {
                continue;
            }
            left -= take;
            let account: Account<SingleId> = item.get_account(env)?;
            let item_partner: Contact<SingleId> = item.get_partner(env)?;
            if partner.is_empty() {
                partner = item_partner.clone();
            }
            let mut counterpart = MapOfFields::default();
            counterpart.insert("account", account.get_id());
            if let Some(id) = item_partner.get_optional_id() {
                counterpart.insert("partner", id);
            }
            counterpart.insert("name", label.clone());
            counterpart.insert("currency", currency.get_id());
            counterpart.insert("amount_currency", -take);
            if take > Decimal::ZERO {
                counterpart.insert("credit", take);
            } else {
                counterpart.insert("debit", -take);
            }
            items.push(counterpart);
            settled.push((item.get_id(), account.get_id()));
        }
        if !left.is_zero() {
            return Err(format!(
                "\"{label}\" is {amount}, and what it settles {}: {left} is left unexplained",
                amount - left
            )
            .into());
        }
        let mut liquidity = MapOfFields::default();
        liquidity.insert("account", bank.get_id());
        if let Some(id) = partner.get_optional_id() {
            liquidity.insert("partner", id);
        }
        liquidity.insert("name", label.clone());
        liquidity.insert("currency", currency.get_id());
        liquidity.insert("amount_currency", amount);
        if amount > Decimal::ZERO {
            liquidity.insert("debit", amount);
        } else {
            liquidity.insert("credit", -amount);
        }
        items.insert(0, liquidity);
        let mut entry = MapOfFields::default();
        entry.insert("journal", journal.get_id());
        entry.insert("date", date);
        entry.insert("reference", label);
        if let Some(id) = partner.get_optional_id() {
            entry.insert("partner", id);
        }
        entry.insert_field_type("lines", FieldType::Commands(vec![Command::Create(items)]));
        let entry: Move<MultipleIds> = env.create_new_records_from_maps(vec![entry])?;
        entry.action_post(env)?;
        let entry: Move<SingleId> = env.get_record(entry.get_ids_ref()[0].into());
        let new_items: MoveLine<MultipleIds> = entry.get_lines(env)?;
        for (settled_item, account) in settled {
            let mut pair = vec![settled_item];
            for new_item in &new_items {
                let new_account: Account<SingleId> = new_item.get_account(env)?;
                if new_account.get_id() == account && !*new_item.get_reconciled(env)? {
                    pair.push(new_item.get_id());
                    break;
                }
            }
            MoveLine::<MultipleIds>::from_ids(pair, env).reconcile_lines(env)?;
        }
        self.set_move_id(&entry, env)?;
        self.set_is_reconciled(true, env)?;
        if !partner.is_empty() {
            self.set_partner(&partner, env)?;
        }
        Ok(())
    }
}
