use crate::models::account::Account;
use crate::models::account_move::MoveState;
use crate::models::account_move_line::MoveLine;
use crate::models::account_trial_balance_line::{BaseAccountTrialBalanceLine, TrialBalanceLine};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::model::ModelVerbs;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use std::collections::BTreeMap;

/// The trial balance of a period: for each account, what it held before, what moved on it, and
/// what it holds at the end — posted entries only.
#[derive(Model)]
#[erp(id = "account_trial_balance", methods)]
#[allow(dead_code)]
pub struct TrialBalance<Mode: IdMode> {
    id: Mode,
    #[erp(label = "From")]
    date_from: NaiveDate,
    #[erp(label = "To")]
    date_to: NaiveDate,
    #[erp(label = "Accounts", inverse = "report", owned)]
    lines: Reference<BaseAccountTrialBalanceLine, MultipleIds>,
    #[erp(label = "Total debit", default = 0.0)]
    total_debit: Decimal,
    #[erp(label = "Total credit", default = 0.0)]
    total_credit: Decimal,
}

/// The rows of a trial balance: `(account, initial balance, debit, credit)` by account code.
pub fn trial_balance(
    env: &mut Environment,
    date_from: NaiveDate,
    date_to: NaiveDate,
) -> Result<Vec<(u32, Decimal, Decimal, Decimal)>> {
    let env = &mut *env.sudo();
    let lines: MoveLine<MultipleIds> = env.search(&make_domain!([
        ("parent_state", "=", MoveState::Posted),
        ("date", "<=", date_to)
    ]))?;
    let mut rows: BTreeMap<String, (u32, Decimal, Decimal, Decimal)> = BTreeMap::new();
    for line in &lines {
        let account: Account<SingleId> = line.get_account(env)?;
        let code = account.get_code(env)?.clone();
        let date = line.get_date(env)?.copied().unwrap_or(date_to);
        let row = rows.entry(code).or_insert((
            account.get_id(),
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        ));
        if date < date_from {
            row.1 += *line.get_balance(env)?;
        } else {
            row.2 += *line.get_debit(env)?;
            row.3 += *line.get_credit(env)?;
        }
    }
    Ok(rows.into_values().collect())
}

#[erp_methods]
impl TrialBalance<MultipleIds> {
    /// Work the trial balance out again, for its period.
    #[erp(rpc)]
    pub fn action_compute(&self, env: &mut Environment) -> Result<bool> {
        for report in self {
            let from = *report.get_date_from(env)?;
            let to = *report.get_date_to(env)?;
            if from > to {
                return Err("The period ends before it starts".into());
            }
            let rows = trial_balance(env, from, to)?;
            let (mut debit_total, mut credit_total) = (Decimal::ZERO, Decimal::ZERO);
            let mut created = Vec::new();
            for (account, initial, debit, credit) in rows {
                debit_total += debit;
                credit_total += credit;
                let mut line = MapOfFields::default();
                line.insert("report", report.get_id());
                line.insert("account", account);
                line.insert("initial_balance", initial);
                line.insert("debit", debit);
                line.insert("credit", credit);
                line.insert("ending_balance", initial + debit - credit);
                created.push(line);
            }
            let old: TrialBalanceLine<MultipleIds> = report.get_lines(env)?;
            old.delete(env)?;
            let _: TrialBalanceLine<MultipleIds> = env.create_new_records_from_maps(created)?;
            report.set_total_debit(debit_total, env)?;
            report.set_total_credit(credit_total, env)?;
        }
        Ok(true)
    }
}
