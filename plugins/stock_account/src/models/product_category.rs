use account::models::{BaseAccount, BaseAccountJournal};
use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};

/// Where the value of a category's stock is kept, and what it moves against.
#[derive(Model)]
#[erp(id = "product_category")]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductCategoryStockAccount<Mode: IdMode> {
    id: Mode,
    #[erp(
        label = "Stock valuation account",
        ondelete = "restrict",
        description = "What the stock is worth; left empty, moves are not booked"
    )]
    stock_valuation_account: Reference<BaseAccount, SingleId>,
    #[erp(
        label = "Stock input account",
        ondelete = "restrict",
        description = "Credited for goods received, debited by their vendor bill"
    )]
    stock_input_account: Reference<BaseAccount, SingleId>,
    #[erp(
        label = "Stock output account",
        ondelete = "restrict",
        description = "Debited for goods leaving the stock: the cost of what is sold"
    )]
    stock_output_account: Reference<BaseAccount, SingleId>,
    #[erp(label = "Stock journal", ondelete = "restrict")]
    stock_journal: Reference<BaseAccountJournal, SingleId>,
}
