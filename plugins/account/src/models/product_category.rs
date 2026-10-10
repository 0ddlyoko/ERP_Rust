use crate::models::account::BaseAccount;
use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};

/// Where the sales and purchases of a category's products are recorded.
#[derive(Model)]
#[erp(id = "product_category")]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductCategoryAccount<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Income account", ondelete = "restrict")]
    income_account: Reference<BaseAccount, SingleId>,
    #[erp(label = "Expense account", ondelete = "restrict")]
    expense_account: Reference<BaseAccount, SingleId>,
}
