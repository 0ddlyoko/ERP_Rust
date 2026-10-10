use crate::models::account_bank_statement_line::{BankStatementLine, BaseAccountBankStatementLine};
use crate::models::account_journal::BaseAccountJournal;
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId};

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
#[erp(id = "account_bank_statement", order = "date desc, id desc", methods)]
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
    #[erp(label = "Status", tracking, readonly)]
    state: StatementState,
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
