use crate::models::account::BaseAccount;
use crate::models::account_fiscal_position::BaseAccountFiscalPosition;
use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};

/// An account replaced by another under a fiscal position.
#[derive(Model)]
#[erp(id = "account_fiscal_position_account")]
#[allow(dead_code)]
pub struct FiscalPositionAccount<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    position: Reference<BaseAccountFiscalPosition, SingleId>,
    #[erp(label = "Account on product", required, ondelete = "cascade")]
    account_src: Reference<BaseAccount, SingleId>,
    #[erp(label = "Account to use instead", required, ondelete = "cascade")]
    account_dest: Reference<BaseAccount, SingleId>,
}
