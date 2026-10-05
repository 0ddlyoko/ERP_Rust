use crate::models::account::BaseAccount;
use crate::models::tax::BaseAccountTax;
use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};

/// The taxes a product is sold and bought with, and where its sales and purchases are recorded
/// when not where its category says.
#[derive(Model)]
#[erp(id = "product")]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductAccount<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Customer taxes", relation = "product_sale_tax_rel")]
    taxes: Reference<BaseAccountTax, MultipleIds>,
    #[erp(label = "Vendor taxes", relation = "product_purchase_tax_rel")]
    supplier_taxes: Reference<BaseAccountTax, MultipleIds>,
    #[erp(label = "Income account", ondelete = "restrict")]
    income_account: Reference<BaseAccount, SingleId>,
    #[erp(label = "Expense account", ondelete = "restrict")]
    expense_account: Reference<BaseAccount, SingleId>,
}

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
