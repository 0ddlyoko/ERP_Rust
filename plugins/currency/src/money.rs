//! Money arithmetic, apart from the records: rounding to a currency and converting at a rate.

use erp::types::field::Decimal;
use rust_decimal::RoundingStrategy;

/// `amount` rounded to a multiple of `rounding` (0.01 for euros), halves away from zero, as
/// invoices and tax returns round. A rounding that is not positive leaves it as it is.
pub fn round(amount: Decimal, rounding: Decimal) -> Decimal {
    if rounding <= Decimal::ZERO {
        return amount;
    }
    let steps =
        (amount / rounding).round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    (steps * rounding).normalize()
}

/// Whether `amount` rounds to zero in a currency rounding to `rounding`.
pub fn is_zero(amount: Decimal, rounding: Decimal) -> bool {
    round(amount, rounding).is_zero()
}

/// `amount` of a currency at `from_rate`, in a currency at `to_rate`, rounded to `rounding`.
///
/// A rate is how much of its currency one unit of the company's currency buys, so the company's
/// own currency is at 1. Errs on a rate that is not positive.
pub fn convert(
    amount: Decimal,
    from_rate: Decimal,
    to_rate: Decimal,
    rounding: Decimal,
) -> Result<Decimal, String> {
    if from_rate <= Decimal::ZERO || to_rate <= Decimal::ZERO {
        return Err(format!(
            "An exchange rate must be positive, not {from_rate} and {to_rate}"
        ));
    }
    if from_rate == to_rate {
        return Ok(round(amount, rounding));
    }
    Ok(round(amount * to_rate / from_rate, rounding))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(value: &str) -> Decimal {
        Decimal::from_str(value).expect("a decimal")
    }

    #[test]
    fn test_round_to_cents() {
        assert_eq!(round(d("10.005"), d("0.01")), d("10.01"));
        assert_eq!(round(d("10.004"), d("0.01")), d("10"));
        assert_eq!(
            round(d("-10.005"), d("0.01")),
            d("-10.01"),
            "a credit note rounds alike"
        );
        assert_eq!(round(d("0"), d("0.01")), d("0"));
        assert_eq!(
            round(d("1234.5"), d("1")),
            d("1235"),
            "a currency without cents"
        );
        assert_eq!(round(d("0.123"), d("0")), d("0.123"));
    }

    #[test]
    fn test_is_zero() {
        assert!(is_zero(d("0.004"), d("0.01")));
        assert!(!is_zero(d("0.005"), d("0.01")));
        assert!(is_zero(d("-0.004"), d("0.01")));
    }

    #[test]
    fn test_convert_at_rates() {
        // 100 EUR at 1.0850 USD for 1 EUR.
        assert_eq!(
            convert(d("100"), d("1"), d("1.0850"), d("0.01")),
            Ok(d("108.5"))
        );
        // Back: 108.50 USD are 100 EUR.
        assert_eq!(
            convert(d("108.50"), d("1.0850"), d("1"), d("0.01")),
            Ok(d("100"))
        );
        // 10 USD at 1.0850 in GBP at 0.8500, both against the euro.
        assert_eq!(
            convert(d("10"), d("1.0850"), d("0.85"), d("0.01")),
            Ok(d("7.83"))
        );
        // A refund converts negative.
        assert_eq!(
            convert(d("-50"), d("1"), d("1.0850"), d("0.01")),
            Ok(d("-54.25"))
        );
        assert_eq!(convert(d("0"), d("1"), d("1.0850"), d("0.01")), Ok(d("0")));
    }

    #[test]
    fn test_a_rate_must_be_positive() {
        assert!(convert(d("1"), d("0"), d("1"), d("0.01")).is_err());
        assert!(convert(d("1"), d("1"), d("-2"), d("0.01")).is_err());
    }
}
