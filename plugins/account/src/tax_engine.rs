//! Taxes on a line, apart from the records: the base, each tax, and where each tax goes.
//!
//! Rounded per line, half away from zero, to the currency's rounding: the line's untaxed amount,
//! then each tax. A tax included in the price is taken out of it so that the untaxed amount and
//! the taxes add up to the price shown.

use currency::money;
use erp::types::field::Decimal;

/// How a tax is worked out from the base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaxKind {
    /// A share of the base: 21 means 21 %.
    Percent,
    /// An amount per unit, whatever the price.
    Fixed,
}

/// Where a share of a tax goes: a factor of it (100, or -100 for the side of a reverse charge
/// owed rather than deducted), to an account, marked for the tax return.
#[derive(Debug, Clone, PartialEq)]
pub struct Repartition {
    pub factor: Decimal,
    pub account: Option<u32>,
    pub tags: Vec<u32>,
}

/// A tax as the engine needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct TaxSpec {
    pub id: u32,
    pub kind: TaxKind,
    pub amount: Decimal,
    pub price_include: bool,
    /// The tags of the base this tax is computed on.
    pub base_tags: Vec<u32>,
    pub repartitions: Vec<Repartition>,
}

/// A line to tax: a unit price, a quantity, a discount in percent.
#[derive(Debug, Clone, PartialEq)]
pub struct LineInput {
    pub price_unit: Decimal,
    pub quantity: Decimal,
    pub discount: Decimal,
    pub taxes: Vec<TaxSpec>,
    pub rounding: Decimal,
}

/// One tax of a line: on what base, how much in all, and how much goes where.
#[derive(Debug, Clone, PartialEq)]
pub struct TaxLine {
    pub tax: u32,
    pub base: Decimal,
    /// What the tax adds to the line: its shares together — nothing for a reverse charge.
    pub amount: Decimal,
    pub shares: Vec<(Repartition, Decimal)>,
}

/// A line taxed: untaxed, taxed, and its taxes.
#[derive(Debug, Clone, PartialEq)]
pub struct LineResult {
    pub subtotal: Decimal,
    pub total: Decimal,
    pub taxes: Vec<TaxLine>,
}

const HUNDRED: Decimal = Decimal::ONE_HUNDRED;

/// The price of one unit once the discount is taken off.
pub fn discounted(price_unit: Decimal, discount: Decimal) -> Decimal {
    price_unit * (HUNDRED - discount) / HUNDRED
}

/// Split `amount` by the factors of `repartitions`: each share rounded, the last positive one
/// taking what rounding left, so the positive shares add up to the amount exactly.
fn split(
    amount: Decimal,
    repartitions: &[Repartition],
    rounding: Decimal,
) -> Vec<(Repartition, Decimal)> {
    let mut shares: Vec<(Repartition, Decimal)> = repartitions
        .iter()
        .map(|repartition| {
            (
                repartition.clone(),
                money::round(amount * repartition.factor / HUNDRED, rounding),
            )
        })
        .collect();
    let positive_factors: Decimal = repartitions
        .iter()
        .map(|repartition| repartition.factor)
        .filter(|factor| *factor > Decimal::ZERO)
        .sum();
    if positive_factors == HUNDRED
        && let Some(last) = shares
            .iter()
            .rposition(|(repartition, _)| repartition.factor > Decimal::ZERO)
    {
        let others: Decimal = shares
            .iter()
            .enumerate()
            .filter(|(index, (repartition, _))| {
                *index != last && repartition.factor > Decimal::ZERO
            })
            .map(|(_, (_, share))| *share)
            .sum();
        shares[last].1 = amount - others;
    }
    shares
}

/// The taxes of `line`, rounded per line.
///
/// Errs on a discount outside 0–100 %: a line is never paid to be taken.
pub fn compute(line: &LineInput) -> Result<LineResult, String> {
    if line.discount < Decimal::ZERO || line.discount > HUNDRED {
        return Err(format!(
            "A discount is between 0 and 100 %, not {} %",
            line.discount
        ));
    }
    let rounding = line.rounding;
    let gross = discounted(line.price_unit, line.discount) * line.quantity;

    let included_fixed: Decimal = line
        .taxes
        .iter()
        .filter(|tax| tax.price_include && tax.kind == TaxKind::Fixed)
        .map(|tax| tax.amount * line.quantity)
        .sum();
    let included_percent: Decimal = line
        .taxes
        .iter()
        .filter(|tax| tax.price_include && tax.kind == TaxKind::Percent)
        .map(|tax| tax.amount)
        .sum();
    let base = (gross - included_fixed) * HUNDRED / (HUNDRED + included_percent);
    let subtotal = money::round(base, rounding);

    let mut taxes = Vec::new();
    let last_included = line.taxes.iter().rposition(|tax| tax.price_include);
    let mut included_so_far = Decimal::ZERO;
    for (index, tax) in line.taxes.iter().enumerate() {
        let raw = match tax.kind {
            TaxKind::Percent => base * tax.amount / HUNDRED,
            TaxKind::Fixed => tax.amount * line.quantity,
        };
        let factor: Decimal = tax
            .repartitions
            .iter()
            .map(|repartition| repartition.factor)
            .sum();
        let mut amount = money::round(raw, rounding);
        if tax.price_include {
            if Some(index) == last_included && factor == HUNDRED {
                // The last included tax takes what rounding left: the price shown stays whole.
                amount = money::round(gross, rounding) - subtotal - included_so_far;
            }
            included_so_far += amount;
        }
        let shares = split(amount, &tax.repartitions, rounding);
        let added = shares.iter().map(|(_, share)| *share).sum();
        taxes.push(TaxLine {
            tax: tax.id,
            base: subtotal,
            amount: added,
            shares,
        });
    }
    let total = subtotal + taxes.iter().map(|tax| tax.amount).sum::<Decimal>();
    Ok(LineResult {
        subtotal,
        total,
        taxes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(value: &str) -> Decimal {
        Decimal::from_str(value).expect("a decimal")
    }

    fn vat(id: u32, rate: &str) -> TaxSpec {
        TaxSpec {
            id,
            kind: TaxKind::Percent,
            amount: d(rate),
            price_include: false,
            base_tags: vec![],
            repartitions: vec![Repartition {
                factor: d("100"),
                account: Some(451),
                tags: vec![54],
            }],
        }
    }

    fn line(price: &str, quantity: &str, discount: &str, taxes: Vec<TaxSpec>) -> LineInput {
        LineInput {
            price_unit: d(price),
            quantity: d(quantity),
            discount: d(discount),
            taxes,
            rounding: d("0.01"),
        }
    }

    #[test]
    fn test_belgian_rates() {
        for (rate, expected_tax) in [("21", "21"), ("12", "12"), ("6", "6"), ("0", "0")] {
            let result = compute(&line("100", "1", "0", vec![vat(1, rate)])).expect("taxed");
            assert_eq!(result.subtotal, d("100"));
            assert_eq!(result.taxes[0].amount, d(expected_tax), "{rate} %");
            assert_eq!(result.total, d("100") + d(expected_tax));
        }
    }

    #[test]
    fn test_quantity_discount_and_rounding() {
        // 3 × 19.99 with 10 % off = 53.973 → 53.97; 21 % of 53.973 = 11.33433 → 11.33.
        let result = compute(&line("19.99", "3", "10", vec![vat(1, "21")])).expect("taxed");
        assert_eq!(result.subtotal, d("53.97"));
        assert_eq!(result.taxes[0].amount, d("11.33"));
        assert_eq!(result.total, d("65.30"));
        // A half cent rounds away from zero: 0.05 × 21 % = 0.0105 → 0.01; 2.5 × 1 cent.
        let result = compute(&line("0.125", "1", "0", vec![])).expect("taxed");
        assert_eq!(result.subtotal, d("0.13"));
    }

    #[test]
    fn test_no_tax_and_zero_quantity() {
        let result = compute(&line("42.50", "2", "0", vec![])).expect("taxed");
        assert_eq!((result.subtotal, result.total), (d("85"), d("85")));
        assert!(result.taxes.is_empty());
        let result = compute(&line("42.50", "0", "0", vec![vat(1, "21")])).expect("taxed");
        assert_eq!((result.subtotal, result.total), (d("0"), d("0")));
        assert_eq!(result.taxes[0].amount, d("0"));
    }

    #[test]
    fn test_negative_amounts_round_alike() {
        // A credit line: the same amounts, negative.
        let result = compute(&line("19.99", "-3", "10", vec![vat(1, "21")])).expect("taxed");
        assert_eq!(result.subtotal, d("-53.97"));
        assert_eq!(result.taxes[0].amount, d("-11.33"));
        assert_eq!(result.total, d("-65.30"));
    }

    #[test]
    fn test_full_discount_and_bounds() {
        let result = compute(&line("100", "1", "100", vec![vat(1, "21")])).expect("taxed");
        assert_eq!(result.total, d("0"));
        assert!(compute(&line("100", "1", "101", vec![])).is_err());
        assert!(compute(&line("100", "1", "-1", vec![])).is_err());
    }

    #[test]
    fn test_price_included_tax_keeps_the_price_whole() {
        let mut included = vat(1, "21");
        included.price_include = true;
        // 10.00 included: 8.264… → 8.26 untaxed, 1.74 tax, 10.00 in all.
        let result = compute(&line("10", "1", "0", vec![included.clone()])).expect("taxed");
        assert_eq!(result.subtotal, d("8.26"));
        assert_eq!(result.taxes[0].amount, d("1.74"));
        assert_eq!(result.total, d("10"));
        // 3 × 0.99 included at 6 %: 2.97 shown, 2.80 + 0.17.
        let mut six = vat(2, "6");
        six.price_include = true;
        let result = compute(&line("0.99", "3", "0", vec![six])).expect("taxed");
        assert_eq!(
            (result.subtotal, result.taxes[0].amount, result.total),
            (d("2.8"), d("0.17"), d("2.97"))
        );
    }

    #[test]
    fn test_fixed_tax_per_unit() {
        let fixed = TaxSpec {
            id: 3,
            kind: TaxKind::Fixed,
            amount: d("0.10"),
            price_include: false,
            base_tags: vec![],
            repartitions: vec![Repartition {
                factor: d("100"),
                account: None,
                tags: vec![],
            }],
        };
        let result = compute(&line("2", "6", "0", vec![fixed, vat(1, "21")])).expect("taxed");
        assert_eq!(result.subtotal, d("12"));
        assert_eq!(result.taxes[0].amount, d("0.6"));
        assert_eq!(result.taxes[1].amount, d("2.52"));
        assert_eq!(result.total, d("15.12"));
    }

    #[test]
    fn test_reverse_charge_adds_nothing_but_moves_both_sides() {
        let reverse = TaxSpec {
            id: 4,
            kind: TaxKind::Percent,
            amount: d("21"),
            price_include: false,
            base_tags: vec![87],
            repartitions: vec![
                Repartition {
                    factor: d("100"),
                    account: Some(411),
                    tags: vec![59],
                },
                Repartition {
                    factor: d("-100"),
                    account: Some(451),
                    tags: vec![56],
                },
            ],
        };
        let result = compute(&line("1000", "1", "0", vec![reverse])).expect("taxed");
        assert_eq!(result.subtotal, d("1000"));
        assert_eq!(
            result.total,
            d("1000"),
            "the supplier is paid the untaxed amount"
        );
        assert_eq!(result.taxes[0].amount, d("0"));
        let shares: Vec<Decimal> = result.taxes[0]
            .shares
            .iter()
            .map(|(_, share)| *share)
            .collect();
        assert_eq!(shares, vec![d("210"), d("-210")]);
    }

    #[test]
    fn test_a_split_tax_adds_up() {
        let half = TaxSpec {
            id: 5,
            kind: TaxKind::Percent,
            amount: d("21"),
            price_include: false,
            base_tags: vec![],
            repartitions: vec![
                Repartition {
                    factor: d("50"),
                    account: Some(411),
                    tags: vec![],
                },
                Repartition {
                    factor: d("50"),
                    account: Some(600),
                    tags: vec![],
                },
            ],
        };
        let shares = |price: &str| -> (Decimal, Vec<Decimal>) {
            let result = compute(&line(price, "1", "0", vec![half.clone()])).expect("taxed");
            let shares = result.taxes[0]
                .shares
                .iter()
                .map(|(_, share)| *share)
                .collect();
            (result.taxes[0].amount, shares)
        };
        // 21 % of 33.33 = 6.9993 → 7.00, split 3.50 + 3.50.
        assert_eq!(shares("33.33"), (d("7"), vec![d("3.5"), d("3.5")]));
        // 21 % of 0.05 = 0.01, split: one cent, never two.
        assert_eq!(shares("0.05"), (d("0.01"), vec![d("0.01"), d("0")]));
    }
}
