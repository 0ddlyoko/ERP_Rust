use crate::models::product_pricelist_item::{
    BaseProductPricelistItem, PricelistCompute, PricelistScope, ProductPricelistItem,
};
use crate::pricing::{self, Pricing, Query, Rule, Scope};
use code_gen::{Model, erp_methods};
use currency::models::BaseCurrency;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId};
use product::models::{Product, ProductCategory};

/// Prices for a kind of customer: rules by product, category, quantity and dates.
#[derive(Model)]
#[erp(id = "product_pricelist", order = "sequence, id", methods)]
#[allow(dead_code)]
pub struct ProductPricelist<Mode: IdMode> {
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

#[erp_methods]
impl ProductPricelist<SingleId> {
    /// The rules of the pricelist, as the pricing engine reads them.
    pub fn rules(&self, env: &mut Environment) -> Result<Vec<Rule>> {
        let env = &mut *env.sudo();
        let items: ProductPricelistItem<MultipleIds> = self.get_items(env)?;
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
