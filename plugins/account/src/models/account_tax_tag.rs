use code_gen::Model;
use erp::types::field::IdMode;

/// A grid of the tax return an amount is reported in: `03`, counted with the sign the entry's
/// balance has on that side, so that grids hold positive amounts.
#[derive(Model)]
#[erp(id = "account_tax_tag", order = "name, id")]
#[allow(dead_code)]
pub struct AccountTaxTag<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(description = "The grid of the tax return, e.g. 03")]
    grid: String,
    #[erp(
        default = 1,
        description = "1 when debits add to the grid, -1 when credits do"
    )]
    sign: i32,
    #[erp(default = true)]
    active: bool,
}
