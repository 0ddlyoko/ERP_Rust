//! Quantity arithmetic, apart from the records: converting between ratios and rounding.

use erp::types::field::Decimal;
use rust_decimal::RoundingStrategy;

/// How a quantity is brought to a precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    /// To the nearest step, halves away from zero: 0.125 → 0.13.
    HalfUp,
    /// To the next step away from zero, for what must not fall short (packs to order).
    Up,
}

/// `value` brought to a multiple of `precision` (0.01, 0.5, 1…). A zero or negative precision
/// leaves the value as it is.
pub fn round_to(value: Decimal, precision: Decimal, rounding: Rounding) -> Decimal {
    if precision <= Decimal::ZERO {
        return value;
    }
    let steps = value / precision;
    let steps = match rounding {
        Rounding::HalfUp => steps.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero),
        Rounding::Up => steps.round_dp_with_strategy(0, RoundingStrategy::AwayFromZero),
    };
    (steps * precision).normalize()
}

/// `quantity`, counted in a unit worth `from_ratio` reference units, counted in one worth
/// `to_ratio`, rounded to `precision`.
///
/// Errs on a ratio that is not positive: no unit is worth nothing.
pub fn convert(
    quantity: Decimal,
    from_ratio: Decimal,
    to_ratio: Decimal,
    precision: Decimal,
    rounding: Rounding,
) -> Result<Decimal, String> {
    if from_ratio <= Decimal::ZERO || to_ratio <= Decimal::ZERO {
        return Err(format!(
            "A unit's ratio must be positive, not {from_ratio} and {to_ratio}"
        ));
    }
    if from_ratio == to_ratio {
        return Ok(round_to(quantity, precision, rounding));
    }
    Ok(round_to(
        quantity * from_ratio / to_ratio,
        precision,
        rounding,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(value: &str) -> Decimal {
        Decimal::from_str(value).expect("a decimal")
    }

    #[test]
    fn test_round_half_up() {
        assert_eq!(round_to(d("0.125"), d("0.01"), Rounding::HalfUp), d("0.13"));
        assert_eq!(round_to(d("0.124"), d("0.01"), Rounding::HalfUp), d("0.12"));
        assert_eq!(
            round_to(d("-0.125"), d("0.01"), Rounding::HalfUp),
            d("-0.13")
        );
        assert_eq!(round_to(d("2.5"), d("1"), Rounding::HalfUp), d("3"));
        assert_eq!(round_to(d("1.26"), d("0.5"), Rounding::HalfUp), d("1.5"));
        assert_eq!(round_to(d("0"), d("0.01"), Rounding::HalfUp), d("0"));
    }

    #[test]
    fn test_round_up() {
        assert_eq!(round_to(d("1.001"), d("1"), Rounding::Up), d("2"));
        assert_eq!(round_to(d("1"), d("1"), Rounding::Up), d("1"));
        assert_eq!(round_to(d("-1.2"), d("1"), Rounding::Up), d("-2"));
    }

    #[test]
    fn test_no_precision_keeps_the_value() {
        assert_eq!(
            round_to(d("1.23456"), d("0"), Rounding::HalfUp),
            d("1.23456")
        );
    }

    #[test]
    fn test_convert_between_ratios() {
        // 3 dozens are 36 units.
        assert_eq!(
            convert(d("3"), d("12"), d("1"), d("1"), Rounding::HalfUp),
            Ok(d("36"))
        );
        // 1500 g are 1.5 kg.
        assert_eq!(
            convert(d("1500"), d("0.001"), d("1"), d("0.001"), Rounding::HalfUp),
            Ok(d("1.5"))
        );
        // 2 lb in kg, to the gram.
        assert_eq!(
            convert(
                d("2"),
                d("0.45359237"),
                d("1"),
                d("0.001"),
                Rounding::HalfUp
            ),
            Ok(d("0.907"))
        );
        // 10 units in dozens, rounded up to whole dozens.
        assert_eq!(
            convert(d("10"), d("1"), d("12"), d("1"), Rounding::Up),
            Ok(d("1"))
        );
        assert_eq!(
            convert(d("13"), d("1"), d("12"), d("1"), Rounding::Up),
            Ok(d("2"))
        );
    }

    #[test]
    fn test_convert_zero_and_negative_quantities() {
        assert_eq!(
            convert(d("0"), d("12"), d("1"), d("1"), Rounding::HalfUp),
            Ok(d("0"))
        );
        assert_eq!(
            convert(d("-2"), d("12"), d("1"), d("1"), Rounding::HalfUp),
            Ok(d("-24"))
        );
    }

    #[test]
    fn test_a_ratio_must_be_positive() {
        assert!(convert(d("1"), d("0"), d("1"), d("1"), Rounding::HalfUp).is_err());
        assert!(convert(d("1"), d("1"), d("-1"), d("1"), Rounding::HalfUp).is_err());
    }
}
