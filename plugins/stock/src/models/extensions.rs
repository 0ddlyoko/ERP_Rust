use crate::models::location::Location;
use crate::models::quant::{BaseStockQuant, Quant};
use crate::models::valuation_layer::{BaseStockValuationLayer, ValuationLayer};
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference};
use erp_search_code_gen::make_domain;

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

/// How much of a product the company holds, and what it is worth.
#[derive(Model)]
#[erp(id = "product", methods)]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductStock<Mode: IdMode> {
    id: Mode,
    #[erp(
        label = "Track inventory",
        default = true,
        tracking,
        description = "Its quantities on hand are kept, counted and valued"
    )]
    is_storable: bool,
    #[erp(label = "Stock", inverse = "product")]
    quants: Reference<BaseStockQuant, MultipleIds>,
    #[erp(label = "Valuation", inverse = "product")]
    valuation_layers: Reference<BaseStockValuationLayer, MultipleIds>,
    #[erp(
        label = "On hand",
        compute = "compute_quantities",
        depends = ["quants.quantity", "quants.reserved_quantity", "valuation_layers.value"]
    )]
    qty_available: Decimal,
    #[erp(
        label = "Free to use",
        compute = "compute_quantities",
        depends = ["quants.quantity", "quants.reserved_quantity", "valuation_layers.value"]
    )]
    free_qty: Decimal,
    #[erp(
        label = "Stock value",
        compute = "compute_quantities",
        depends = ["quants.quantity", "quants.reserved_quantity", "valuation_layers.value"]
    )]
    stock_value: Decimal,
}

#[erp_methods]
impl ProductStock<MultipleIds> {
    /// What is in the company's stock locations, what of it is not promised, and its value.
    pub fn compute_quantities(&self, env: &mut Environment) -> Result<()> {
        let internal: Vec<u32> = {
            let env = &mut *env.sudo();
            let found: Location<MultipleIds> =
                env.search(&make_domain!([("usage", "=", "internal")]))?;
            found.get_ids_ref().clone()
        };
        for product in self {
            let (on_hand, free) = {
                let quants = Quant::at(env, product.get_id(), internal.clone())?;
                let env = &mut *env.sudo();
                let mut on_hand = Decimal::ZERO;
                let mut free = Decimal::ZERO;
                for quant in &quants {
                    let quantity = *quant.get_quantity(env)?;
                    on_hand += quantity;
                    free += quantity - *quant.get_reserved_quantity(env)?;
                }
                (on_hand, free)
            };
            let value = ValuationLayer::value_of(env, product.get_id())?;
            product.set_qty_available(on_hand, env)?;
            product.set_free_qty(free, env)?;
            product.set_stock_value(value, env)?;
        }
        Ok(())
    }
}
