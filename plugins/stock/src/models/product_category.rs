use code_gen::{Model, selection};
use erp::types::field::IdMode;

#[selection]
pub enum StockCostMethod {
    #[selection(label = "Standard price")]
    Standard,
    #[default]
    #[selection(label = "Average cost")]
    Average,
    #[selection(label = "First in, first out")]
    Fifo,
}

/// How the stock of a category's products is valued.
#[derive(Model)]
#[erp(id = "product_category")]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductCategoryStock<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Costing method")]
    cost_method: StockCostMethod,
}
