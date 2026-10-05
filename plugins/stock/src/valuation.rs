//! Inventory valuation, apart from the records: what goods coming in cost, and what goods going
//! out are taken at, by standard price, average cost, or first in first out.

use erp::types::field::Decimal;

/// How a product's stock is valued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostMethod {
    /// At the product's cost, whatever was paid.
    Standard,
    /// At the average of what the units in stock cost.
    Average,
    /// What goes out is what came in first, at what it cost.
    Fifo,
}

/// Units that came in at a cost and are not all gone yet, oldest first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layer {
    pub id: u32,
    pub remaining_quantity: Decimal,
    pub remaining_value: Decimal,
}

/// What a receipt is valued at: its own unit cost, or the standard cost.
pub fn incoming_value(
    method: CostMethod,
    quantity: Decimal,
    unit_cost: Decimal,
    standard: Decimal,
    rounding: Decimal,
) -> Decimal {
    let cost = match method {
        CostMethod::Standard => standard,
        _ => unit_cost,
    };
    currency::money::round(quantity * cost, rounding)
}

/// The average cost once `quantity` more units come in at `unit_cost`, `on_hand` being in
/// stock at `average` before. Nothing in stock after it, the last cost.
pub fn new_average(
    on_hand: Decimal,
    average: Decimal,
    quantity: Decimal,
    unit_cost: Decimal,
) -> Decimal {
    let total = on_hand + quantity;
    if total <= Decimal::ZERO {
        return unit_cost;
    }
    // Stock below nothing is worth nothing more: only what is there counts.
    let kept = on_hand.max(Decimal::ZERO);
    ((kept * average + quantity * unit_cost) / (kept + quantity)).round_dp(6)
}

/// What taking `quantity` out of the layers is worth, and how much is taken from each: oldest
/// first. Taking more than the layers hold takes the rest at the last cost known.
pub fn fifo_out(
    layers: &[Layer],
    quantity: Decimal,
    last_cost: Decimal,
    rounding: Decimal,
) -> (Decimal, Vec<(u32, Decimal, Decimal)>) {
    let mut left = quantity;
    let mut value = Decimal::ZERO;
    let mut taken = Vec::new();
    for layer in layers {
        if left <= Decimal::ZERO {
            break;
        }
        if layer.remaining_quantity <= Decimal::ZERO {
            continue;
        }
        let take = left.min(layer.remaining_quantity);
        let take_value = if take == layer.remaining_quantity {
            layer.remaining_value
        } else {
            currency::money::round(
                layer.remaining_value * take / layer.remaining_quantity,
                rounding,
            )
        };
        value += take_value;
        left -= take;
        taken.push((layer.id, take, take_value));
    }
    if left > Decimal::ZERO {
        value += currency::money::round(left * last_cost, rounding);
    }
    (value, taken)
}

/// What a delivery of `quantity` is taken out of stock at, as a positive value.
pub fn outgoing_value(
    method: CostMethod,
    quantity: Decimal,
    average: Decimal,
    standard: Decimal,
    rounding: Decimal,
) -> Decimal {
    let cost = match method {
        CostMethod::Standard => standard,
        _ => average,
    };
    currency::money::round(quantity * cost, rounding)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(value: &str) -> Decimal {
        Decimal::from_str(value).expect("a decimal")
    }

    #[test]
    fn test_incoming() {
        assert_eq!(
            incoming_value(CostMethod::Average, d("10"), d("4.5"), d("5"), d("0.01")),
            d("45")
        );
        assert_eq!(
            incoming_value(CostMethod::Fifo, d("3"), d("1.333"), d("5"), d("0.01")),
            d("4")
        );
        assert_eq!(
            incoming_value(CostMethod::Standard, d("10"), d("4.5"), d("5"), d("0.01")),
            d("50")
        );
        assert_eq!(
            incoming_value(CostMethod::Average, d("0"), d("4.5"), d("5"), d("0.01")),
            d("0")
        );
    }

    #[test]
    fn test_average_cost() {
        // 10 at 4 then 10 at 6: 5.
        assert_eq!(new_average(d("10"), d("4"), d("10"), d("6")), d("5"));
        // Nothing yet: the cost of the first receipt.
        assert_eq!(new_average(d("0"), d("0"), d("8"), d("3.25")), d("3.25"));
        // 3 at 10 then 7 at 11: 10.7.
        assert_eq!(new_average(d("3"), d("10"), d("7"), d("11")), d("10.7"));
        // 1 at 1 and 2 at 2: 1.666667.
        assert_eq!(new_average(d("1"), d("1"), d("2"), d("2")), d("1.666667"));
        // Below nothing, only the new units count.
        assert_eq!(new_average(d("-2"), d("9"), d("5"), d("4")), d("4"));
    }

    #[test]
    fn test_outgoing() {
        assert_eq!(
            outgoing_value(
                CostMethod::Average,
                d("3"),
                d("1.666667"),
                d("9"),
                d("0.01")
            ),
            d("5")
        );
        assert_eq!(
            outgoing_value(CostMethod::Standard, d("3"), d("1.66"), d("9"), d("0.01")),
            d("27")
        );
        assert_eq!(
            outgoing_value(CostMethod::Average, d("0"), d("5"), d("9"), d("0.01")),
            d("0")
        );
    }

    #[test]
    fn test_fifo() {
        let layers = [
            Layer {
                id: 1,
                remaining_quantity: d("10"),
                remaining_value: d("40"),
            },
            Layer {
                id: 2,
                remaining_quantity: d("10"),
                remaining_value: d("60"),
            },
        ];
        // 15 out: 10 at 4 and 5 at 6.
        let (value, taken) = fifo_out(&layers, d("15"), d("6"), d("0.01"));
        assert_eq!(value, d("70"));
        assert_eq!(taken, vec![(1, d("10"), d("40")), (2, d("5"), d("30"))]);
        // 25 out: all of it, and 5 more at the last cost.
        let (value, _) = fifo_out(&layers, d("25"), d("6"), d("0.01"));
        assert_eq!(value, d("130"));
        // A third of a layer of 10 units worth 10: rounded to the cent.
        let thirds = [Layer {
            id: 3,
            remaining_quantity: d("3"),
            remaining_value: d("10"),
        }];
        let (value, _) = fifo_out(&thirds, d("1"), d("3.33"), d("0.01"));
        assert_eq!(value, d("3.33"));
        let (value, _) = fifo_out(&thirds, d("3"), d("3.33"), d("0.01"));
        assert_eq!(value, d("10"), "the whole layer, to the cent");
    }
}
