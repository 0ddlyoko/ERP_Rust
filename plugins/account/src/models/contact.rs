use crate::models::account::BaseAccount;
use crate::models::account_fiscal_position::BaseAccountFiscalPosition;
use crate::models::account_payment_term::BaseAccountPaymentTerm;
use crate::models::contact_bank::BaseContactBank;
use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};

/// What invoicing a contact needs: its terms, its fiscal position, its bank accounts, and the
/// accounts it is recorded on when they differ from the company's.
#[derive(Model)]
#[erp(id = "contact")]
#[erp(derived_model = "base::models")]
#[allow(dead_code)]
pub struct ContactAccount<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Customer payment terms", ondelete = "set_null")]
    customer_payment_term: Reference<BaseAccountPaymentTerm, SingleId>,
    #[erp(label = "Supplier payment terms", ondelete = "set_null")]
    supplier_payment_term: Reference<BaseAccountPaymentTerm, SingleId>,
    #[erp(label = "Fiscal position", ondelete = "set_null", tracking)]
    fiscal_position: Reference<BaseAccountFiscalPosition, SingleId>,
    #[erp(label = "Customer account", ondelete = "restrict")]
    account_receivable: Reference<BaseAccount, SingleId>,
    #[erp(label = "Supplier account", ondelete = "restrict")]
    account_payable: Reference<BaseAccount, SingleId>,
    #[erp(label = "Bank accounts", inverse = "contact", owned)]
    bank_accounts: Reference<BaseContactBank, MultipleIds>,
}
