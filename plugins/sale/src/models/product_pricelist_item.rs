use crate::models::product_pricelist::BaseProductPricelist;
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId};
use product::models::{BaseProduct, BaseProductCategory, Product, ProductCategory};

#[selection]
pub enum PricelistScope {
    #[default]
    #[selection(label = "All products")]
    All,
    #[selection(label = "Product category")]
    Category,
    #[selection(label = "Product")]
    Product,
}

#[selection]
pub enum PricelistCompute {
    #[default]
    #[selection(label = "Discount on the sales price")]
    Discount,
    #[selection(label = "Fixed price")]
    Fixed,
}

/// A rule of a pricelist.
#[derive(Model)]
#[erp(id = "product_pricelist_item", order = "sequence, id", methods)]
#[allow(dead_code)]
pub struct ProductPricelistItem<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    pricelist: Reference<BaseProductPricelist, SingleId>,
    #[erp(label = "Apply on")]
    applied_on: PricelistScope,
    #[erp(ondelete = "cascade")]
    product: Reference<BaseProduct, SingleId>,
    #[erp(label = "Category", ondelete = "cascade")]
    category: Reference<BaseProductCategory, SingleId>,
    #[erp(label = "Minimum quantity", default = 0.0)]
    min_quantity: Decimal,
    #[erp(label = "Start date")]
    date_start: Option<NaiveDate>,
    #[erp(label = "End date")]
    date_end: Option<NaiveDate>,
    #[erp(label = "Computation")]
    compute_price: PricelistCompute,
    #[erp(label = "Fixed price", default = 0.0)]
    fixed_price: Decimal,
    #[erp(label = "Discount (%)", default = 0.0)]
    percent_price: Decimal,
    #[erp(default = 10)]
    sequence: i32,
}

#[erp_methods]
impl ProductPricelistItem<MultipleIds> {
    /// A rule names what it applies to; a discount stays within 0 and 100 %; a fixed price and a
    /// quantity are not negative; a rule ends after it starts.
    #[erp(check = ["applied_on", "category", "product", "date_start", "date_end", "fixed_price", "percent_price", "min_quantity"])]
    pub fn check_items(&self, env: &mut Environment) -> Result<()> {
        for item in self {
            let product: Product<SingleId> = item.get_product(env)?;
            let category: ProductCategory<SingleId> = item.get_category(env)?;
            match *item.get_applied_on(env)? {
                PricelistScope::Product if product.is_empty() => {
                    return Err("A pricelist rule on a product names the product".into());
                }
                PricelistScope::Category if category.is_empty() => {
                    return Err("A pricelist rule on a category names the category".into());
                }
                _ => {}
            }
            let percent = *item.get_percent_price(env)?;
            if percent < Decimal::ZERO || percent > Decimal::ONE_HUNDRED {
                return Err(format!("A discount is between 0 and 100 %, not {percent} %").into());
            }
            if *item.get_fixed_price(env)? < Decimal::ZERO
                || *item.get_min_quantity(env)? < Decimal::ZERO
            {
                return Err("A pricelist rule has no negative price nor quantity".into());
            }
            if let (Some(start), Some(end)) = (
                item.get_date_start(env)?.copied(),
                item.get_date_end(env)?.copied(),
            ) && end < start
            {
                return Err(
                    format!("A pricelist rule cannot end on {end}, before it starts").into(),
                );
            }
        }
        Ok(())
    }
}
