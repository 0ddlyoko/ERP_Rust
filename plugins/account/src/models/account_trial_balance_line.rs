use crate::models::account::BaseAccount;
use crate::models::account_trial_balance::BaseAccountTrialBalance;
use code_gen::Model;
use erp::types::field::{Decimal, IdMode, Reference, SingleId};

/// One account of a trial balance.
#[derive(Model)]
#[erp(id = "account_trial_balance_line")]
#[allow(dead_code)]
pub struct TrialBalanceLine<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    report: Reference<BaseAccountTrialBalance, SingleId>,
    #[erp(required, ondelete = "cascade")]
    account: Reference<BaseAccount, SingleId>,
    #[erp(label = "Initial balance", default = 0.0)]
    initial_balance: Decimal,
    #[erp(default = 0.0)]
    debit: Decimal,
    #[erp(default = 0.0)]
    credit: Decimal,
    #[erp(label = "Ending balance", default = 0.0)]
    ending_balance: Decimal,
}
