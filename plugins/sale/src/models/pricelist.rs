use crate::pricing::{self, Pricing, Query, Rule, Scope};
use code_gen::{Model, erp_methods, selection};
use currency::models::BaseCurrency;
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

/// Prices for a kind of customer: rules by product, category, quantity and dates.
#[derive(Model)]
#[erp(id = "product_pricelist", order = "sequence, id", methods)]
#[allow(dead_code)]
pub struct Pricelist<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(ondelete = "restrict", description = "Left empty, the company's")]
    currency: Reference<BaseCurrency, SingleId>,
    #[erp(label = "Rules", inverse = "pricelist", owned)]
    items: Reference<BaseProductPricelistItem, MultipleIds>,
    #[erp(default = 10)]
    sequence: i32,
    #[erp(default = true)]
    active: bool,
}

/// A rule of a pricelist.
#[derive(Model)]
#[erp(id = "product_pricelist_item", order = "sequence, id", methods)]
#[allow(dead_code)]
pub struct PricelistItem<Mode: IdMode> {
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
impl PricelistItem<MultipleIds> {
    /// A rule names what it applies to; a discount stays within 0 and 100 %; a fixed price and a
    /// quantity are not negative; a rule ends after it starts.
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

#[erp_methods]
impl Pricelist<SingleId> {
    /// The rules of the pricelist, as the pricing engine reads them.
    pub fn rules(&self, env: &mut Environment) -> Result<Vec<Rule>> {
        let env = &mut *env.sudo();
        let items: PricelistItem<MultipleIds> = self.get_items(env)?;
        let mut rules = Vec::new();
        for item in &items {
            let product: Product<SingleId> = item.get_product(env)?;
            let category: ProductCategory<SingleId> = item.get_category(env)?;
            let scope = match *item.get_applied_on(env)? {
                PricelistScope::Product => Scope::Product(product.get_id()),
                PricelistScope::Category => Scope::Category(category.get_id()),
                _ => Scope::All,
            };
            let pricing = match *item.get_compute_price(env)? {
                PricelistCompute::Fixed => Pricing::Fixed(*item.get_fixed_price(env)?),
                _ => Pricing::Discount(*item.get_percent_price(env)?),
            };
            rules.push(Rule {
                scope,
                min_quantity: *item.get_min_quantity(env)?,
                date_start: item.get_date_start(env)?.copied(),
                date_end: item.get_date_end(env)?.copied(),
                pricing,
                sequence: *item.get_sequence(env)?,
            });
        }
        Ok(rules)
    }

    /// The unit price of `product` for `quantity` on `date`, in the product's unit: the
    /// pricelist's rule, else the product's sales price. No pricelist, the sales price.
    pub fn price_of(
        &self,
        env: &mut Environment,
        product: Product<SingleId>,
        quantity: Decimal,
        date: NaiveDate,
    ) -> Result<Decimal> {
        let env = &mut *env.sudo();
        let list_price = *product.get_list_price(env)?;
        if self.is_empty() {
            return Ok(list_price);
        }
        let mut categories = Vec::new();
        let mut category: ProductCategory<SingleId> = product.get_category(env)?;
        while !category.is_empty() && !categories.contains(&category.get_id()) {
            categories.push(category.get_id());
            category = category.get_parent(env)?;
        }
        let rules = self.rules(env)?;
        Ok(pricing::price(
            &rules,
            &Query {
                product: product.get_id(),
                categories,
                quantity,
                date,
                list_price,
            },
        ))
    }
}
