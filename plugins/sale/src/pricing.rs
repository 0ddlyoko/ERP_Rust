//! Prices from a pricelist, apart from the records: which rule applies to a product, for a
//! quantity, on a date, and the price it makes.

use erp::types::field::{Decimal, NaiveDate};

/// What a pricelist rule applies to: the most specific rule wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    All,
    Category(u32),
    Product(u32),
}

/// How a rule prices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pricing {
    /// This price, whatever the product's.
    Fixed(Decimal),
    /// The product's sales price less this percentage.
    Discount(Decimal),
}

/// One rule of a pricelist.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub scope: Scope,
    pub min_quantity: Decimal,
    pub date_start: Option<NaiveDate>,
    pub date_end: Option<NaiveDate>,
    pub pricing: Pricing,
    pub sequence: i32,
}

/// What is being priced: the product, the categories it falls in (its own first, then its
/// parents), the quantity and the date.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub product: u32,
    pub categories: Vec<u32>,
    pub quantity: Decimal,
    pub date: NaiveDate,
    pub list_price: Decimal,
}

/// How specific a rule is for the query: a product's own rule, then its category's (nearer
/// first), then a rule for all; `None` when the rule does not apply to the product.
fn specificity(rule: &Rule, query: &Query) -> Option<usize> {
    match rule.scope {
        Scope::Product(product) => (product == query.product).then_some(0),
        Scope::Category(category) => query
            .categories
            .iter()
            .position(|candidate| *candidate == category)
            .map(|depth| 1 + depth),
        Scope::All => Some(usize::MAX),
    }
}

/// The rule applying to the query: in force on its date, for at least its quantity, the most
/// specific; among those, the one for the largest quantity, then the first in sequence.
pub fn applicable<'a>(rules: &'a [Rule], query: &Query) -> Option<&'a Rule> {
    rules
        .iter()
        .filter(|rule| rule.min_quantity <= query.quantity)
        .filter(|rule| rule.date_start.is_none_or(|start| start <= query.date))
        .filter(|rule| rule.date_end.is_none_or(|end| query.date <= end))
        .filter_map(|rule| specificity(rule, query).map(|rank| (rank, rule)))
        .min_by(|(rank_a, a), (rank_b, b)| {
            rank_a
                .cmp(rank_b)
                .then(b.min_quantity.cmp(&a.min_quantity))
                .then(a.sequence.cmp(&b.sequence))
        })
        .map(|(_, rule)| rule)
}

/// The unit price of the query on these rules: the applicable rule's, else the list price.
pub fn price(rules: &[Rule], query: &Query) -> Decimal {
    match applicable(rules, query).map(|rule| rule.pricing) {
        Some(Pricing::Fixed(price)) => price,
        Some(Pricing::Discount(percent)) => {
            query.list_price * (Decimal::ONE_HUNDRED - percent) / Decimal::ONE_HUNDRED
        }
        None => query.list_price,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(value: &str) -> Decimal {
        Decimal::from_str(value).expect("a decimal")
    }

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date")
    }

    fn rule(scope: Scope, min: &str, pricing: Pricing) -> Rule {
        Rule {
            scope,
            min_quantity: d(min),
            date_start: None,
            date_end: None,
            pricing,
            sequence: 10,
        }
    }

    fn query(quantity: &str, on: &str) -> Query {
        Query {
            product: 7,
            categories: vec![3, 1],
            quantity: d(quantity),
            date: date(on),
            list_price: d("100"),
        }
    }

    #[test]
    fn test_no_rule_keeps_the_list_price() {
        assert_eq!(price(&[], &query("1", "2026-01-01")), d("100"));
        let other = [rule(Scope::Product(8), "0", Pricing::Fixed(d("1")))];
        assert_eq!(price(&other, &query("1", "2026-01-01")), d("100"));
    }

    #[test]
    fn test_the_most_specific_rule_wins() {
        let rules = [
            rule(Scope::All, "0", Pricing::Discount(d("5"))),
            rule(Scope::Category(1), "0", Pricing::Discount(d("10"))),
            rule(Scope::Category(3), "0", Pricing::Discount(d("15"))),
        ];
        assert_eq!(
            price(&rules, &query("1", "2026-01-01")),
            d("85"),
            "its own category"
        );
        let mut with_product = rules.to_vec();
        with_product.push(rule(Scope::Product(7), "0", Pricing::Fixed(d("79.90"))));
        assert_eq!(price(&with_product, &query("1", "2026-01-01")), d("79.90"));
    }

    #[test]
    fn test_quantity_breaks() {
        let rules = [
            rule(Scope::Product(7), "0", Pricing::Fixed(d("100"))),
            rule(Scope::Product(7), "10", Pricing::Fixed(d("90"))),
            rule(Scope::Product(7), "100", Pricing::Fixed(d("75"))),
        ];
        assert_eq!(price(&rules, &query("9", "2026-01-01")), d("100"));
        assert_eq!(price(&rules, &query("10", "2026-01-01")), d("90"));
        assert_eq!(price(&rules, &query("250", "2026-01-01")), d("75"));
        assert_eq!(
            price(&rules, &query("0", "2026-01-01")),
            d("100"),
            "nothing ordered yet"
        );
    }

    #[test]
    fn test_dates() {
        let mut sale = rule(Scope::All, "0", Pricing::Discount(d("20")));
        sale.date_start = Some(date("2026-07-01"));
        sale.date_end = Some(date("2026-07-31"));
        let rules = [sale];
        assert_eq!(price(&rules, &query("1", "2026-06-30")), d("100"));
        assert_eq!(price(&rules, &query("1", "2026-07-01")), d("80"));
        assert_eq!(price(&rules, &query("1", "2026-07-31")), d("80"));
        assert_eq!(price(&rules, &query("1", "2026-08-01")), d("100"));
    }

    #[test]
    fn test_a_full_discount() {
        let rules = [rule(Scope::All, "0", Pricing::Discount(d("100")))];
        assert_eq!(price(&rules, &query("1", "2026-01-01")), d("0"));
    }
}
