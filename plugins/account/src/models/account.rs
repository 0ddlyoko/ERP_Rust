use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::Selection;
use erp::types::field::{IdMode, MultipleIds, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;

#[selection]
pub enum AccountType {
    #[selection(label = "Receivable")]
    AssetReceivable,
    #[selection(label = "Bank and cash")]
    AssetCash,
    #[default]
    #[selection(label = "Current assets")]
    AssetCurrent,
    #[selection(label = "Non-current assets")]
    AssetNonCurrent,
    #[selection(label = "Fixed assets")]
    AssetFixed,
    #[selection(label = "Payable")]
    LiabilityPayable,
    #[selection(label = "Current liabilities")]
    LiabilityCurrent,
    #[selection(label = "Non-current liabilities")]
    LiabilityNonCurrent,
    #[selection(label = "Equity")]
    Equity,
    #[selection(label = "Income")]
    Income,
    #[selection(label = "Other income")]
    IncomeOther,
    #[selection(label = "Expenses")]
    Expense,
    #[selection(label = "Depreciation")]
    ExpenseDepreciation,
    #[selection(label = "Off-balance sheet")]
    OffBalance,
}

impl AccountType {
    /// Whether amounts on accounts of this type are left open until paid: receivables and
    /// payables.
    pub fn is_receivable_or_payable(self) -> bool {
        matches!(
            self,
            AccountType::AssetReceivable | AccountType::LiabilityPayable
        )
    }

    /// Whether the type belongs to the profit and loss rather than the balance sheet.
    pub fn is_profit_and_loss(self) -> bool {
        matches!(
            self,
            AccountType::Income
                | AccountType::IncomeOther
                | AccountType::Expense
                | AccountType::ExpenseDepreciation
        )
    }
}

/// An account of the chart: `400000 Customers`, where amounts are recorded.
#[derive(Model)]
#[erp(id = "account", name_field = "display_name", methods)]
#[allow(dead_code)]
pub struct Account<Mode: IdMode> {
    id: Mode,
    code: String,
    name: String,
    #[erp(
        label = "Account",
        compute = "compute_display_name",
        depends = ["code", "name"],
        stored
    )]
    display_name: String,
    #[erp(label = "Type", tracking)]
    account_type: AccountType,
    #[erp(
        label = "Allow reconciliation",
        description = "Entries on it are matched with each other: invoices with their payments"
    )]
    reconcile: bool,
    note: Option<String>,
    #[erp(default = true)]
    active: bool,
}

impl Account<SingleId> {
    /// The active account of `code`, empty when there is none.
    pub fn by_code(env: &mut Environment, code: &str) -> Result<Account<SingleId>> {
        let env = &mut *env.sudo();
        let found: Account<MultipleIds> = env.search(&make_domain!([("code", "=", code)]))?;
        Ok(found
            .into_iter()
            .next()
            .unwrap_or_else(|| env.get_record(SingleId::empty())))
    }
}

#[erp_methods]
impl Account<MultipleIds> {
    /// A code is unique; a receivable or payable account is reconciled.
    pub fn check_accounts(&self, env: &mut Environment) -> Result<()> {
        for account in self {
            let code = account.get_code(env)?.trim().to_string();
            if code.is_empty() {
                return Err("An account needs a code".into());
            }
            let same = env
                .sudo()
                .count("account", &make_domain!([("code", "=", code.clone())]))?;
            if same > 1 {
                return Err(format!("The account code {code} is already used").into());
            }
            if account.get_account_type(env)?.is_receivable_or_payable()
                && !*account.get_reconcile(env)?
            {
                return Err(format!(
                    "The account {code} holds receivables or payables: it must allow reconciliation"
                )
                .into());
            }
        }
        Ok(())
    }

    /// `400000 Customers`.
    pub fn compute_display_name(&self, env: &mut Environment) -> Result<()> {
        for account in self {
            let code = account.get_code(env)?.clone();
            let display = format!("{code} {}", account.get_name(env)?);
            account.set_display_name(display, env)?;
        }
        Ok(())
    }

    /// Receivables and payables are always reconciled: that is how invoices learn they are
    /// paid.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let mut values = values;
        for account in &mut values {
            let kind = account
                .get_option::<&String>("account_type")
                .map(|kind| AccountType::from_key(kind));
            if kind.is_some_and(AccountType::is_receivable_or_payable) {
                account.insert("reconcile", true);
            }
        }
        env.savepoint(|env| {
            let ids: MultipleIds = sup.call_with(values, env)?;
            Account::<MultipleIds>::from_ids(ids.clone(), env).check_accounts(env)?;
            Ok(ids)
        })
    }

    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        env.savepoint(|env| {
            sup.call_with(values, env)?;
            self.check_accounts(env)
        })
    }
}
