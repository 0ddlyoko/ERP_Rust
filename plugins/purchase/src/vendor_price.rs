//! The price a vendor asks, apart from the records: of the vendor's prices for a product, the
//! one for the largest minimum quantity reached.

use erp::types::field::Decimal;

/// A price a vendor asks from a quantity on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VendorPrice {
    pub id: u32,
    pub vendor: u32,
    pub min_quantity: Decimal,
    pub price: Decimal,
    pub sequence: i32,
}

/// The price `vendor` asks for `quantity`: of its prices, the one for the largest minimum the
/// quantity reaches, first in sequence; none when it asks none for so few.
pub fn best_price(prices: &[VendorPrice], vendor: u32, quantity: Decimal) -> Option<VendorPrice> {
    prices
        .iter()
        .filter(|price| price.vendor == vendor && price.min_quantity <= quantity)
        .min_by(|a, b| {
            b.min_quantity
                .cmp(&a.min_quantity)
                .then(a.sequence.cmp(&b.sequence))
                .then(a.id.cmp(&b.id))
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(value: &str) -> Decimal {
        Decimal::from_str(value).expect("a decimal")
    }

    fn price(id: u32, vendor: u32, min: &str, price: &str) -> VendorPrice {
        VendorPrice {
            id,
            vendor,
            min_quantity: d(min),
            price: d(price),
            sequence: 10,
        }
    }

    #[test]
    fn test_quantity_breaks_of_one_vendor() {
        let prices = [
            price(1, 7, "0", "10"),
            price(2, 7, "100", "8"),
            price(3, 9, "0", "6"),
        ];
        assert_eq!(
            best_price(&prices, 7, d("5")).map(|p| p.price),
            Some(d("10"))
        );
        assert_eq!(
            best_price(&prices, 7, d("100")).map(|p| p.price),
            Some(d("8"))
        );
        assert_eq!(
            best_price(&prices, 9, d("1")).map(|p| p.price),
            Some(d("6")),
            "the other vendor's"
        );
        assert_eq!(
            best_price(&prices, 8, d("1")),
            None,
            "a vendor asking nothing"
        );
    }

    #[test]
    fn test_below_every_minimum() {
        let prices = [price(1, 7, "10", "9")];
        assert_eq!(best_price(&prices, 7, d("3")), None);
    }
}
