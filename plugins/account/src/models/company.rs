use crate::models::account::{Account, BaseAccount};
use crate::models::account_journal::{AccountJournal, BaseAccountJournal};
use crate::models::account_tax::BaseAccountTax;
use base::models::Company;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, NaiveDate, Reference, SingleId};

/// What the company's books default to: the accounts customers and suppliers are recorded on,
/// the taxes new products get, and the date before which nothing may change.
#[derive(Model)]
#[erp(id = "company", methods)]
#[erp(derived_model = "base::models")]
#[allow(dead_code)]
pub struct CompanyAccount<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Customer account", ondelete = "restrict")]
    account_receivable: Reference<BaseAccount, SingleId>,
    #[erp(label = "Supplier account", ondelete = "restrict")]
    account_payable: Reference<BaseAccount, SingleId>,
    #[erp(label = "Default income account", ondelete = "restrict")]
    account_income: Reference<BaseAccount, SingleId>,
    #[erp(label = "Default expense account", ondelete = "restrict")]
    account_expense: Reference<BaseAccount, SingleId>,
    #[erp(label = "Exchange gain account", ondelete = "restrict")]
    account_exchange_gain: Reference<BaseAccount, SingleId>,
    #[erp(label = "Exchange loss account", ondelete = "restrict")]
    account_exchange_loss: Reference<BaseAccount, SingleId>,
    #[erp(label = "Exchange difference journal", ondelete = "restrict")]
    journal_exchange: Reference<BaseAccountJournal, SingleId>,
    #[erp(label = "Default sales tax", ondelete = "restrict")]
    sale_tax: Reference<BaseAccountTax, SingleId>,
    #[erp(label = "Default purchase tax", ondelete = "restrict")]
    purchase_tax: Reference<BaseAccountTax, SingleId>,
    #[erp(
        label = "Lock date",
        tracking,
        description = "Entries dated on or before it can no longer be posted, changed or cancelled"
    )]
    lock_date: Option<NaiveDate>,
}

#[erp_methods]
impl CompanyAccount<SingleId> {
    /// One of the company's default accounts, as sudo; errs naming it when it is not set, as
    /// nothing can be recorded without it.
    pub fn required_account(
        &self,
        env: &mut Environment,
        which: String,
    ) -> Result<Account<SingleId>> {
        let env = &mut *env.sudo();
        let account: Account<SingleId> = match which.as_str() {
            "receivable" => self.get_account_receivable(env)?,
            "payable" => self.get_account_payable(env)?,
            "income" => self.get_account_income(env)?,
            "expense" => self.get_account_expense(env)?,
            "exchange_gain" => self.get_account_exchange_gain(env)?,
            "exchange_loss" => self.get_account_exchange_loss(env)?,
            _ => env.get_record(SingleId::empty()),
        };
        if account.is_empty() {
            return Err(format!(
                "The company has no default {} account: set it in the accounting settings",
                which.replace('_', " ")
            )
            .into());
        }
        Ok(account)
    }

    /// The journal exchange differences are recorded in.
    pub fn exchange_journal(&self, env: &mut Environment) -> Result<AccountJournal<SingleId>> {
        self.get_journal_exchange(&mut env.sudo())
    }

    /// Refuse touching an entry dated `date` when the books are locked up to it.
    pub fn check_lock(&self, env: &mut Environment, date: NaiveDate) -> Result<()> {
        let env = &mut *env.sudo();
        if let Some(lock) = self.get_lock_date(env)?.copied()
            && date <= lock
        {
            return Err(format!(
                "The books are locked up to {lock}: nothing dated {date} may change"
            )
            .into());
        }
        Ok(())
    }
}

impl CompanyAccount<SingleId> {
    /// The accounting settings of the current company.
    pub fn current(env: &mut Environment) -> Result<CompanyAccount<SingleId>> {
        let company = Company::current(env)?;
        Ok(company.as_model())
    }
}
