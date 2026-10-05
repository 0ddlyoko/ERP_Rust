//! When an invoice falls due, apart from the records: installments by percentage, then the
//! balance, each so many days after the invoice, at the end of the month when asked.

use chrono::{Datelike, Months, NaiveDate};
use currency::money;
use erp::types::field::{Decimal, TimeDelta};

/// How much one installment is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallmentValue {
    /// A share of the total, 30 meaning 30 %.
    Percent,
    /// What is left once the other installments are taken.
    Balance,
}

/// One installment of a payment term.
#[derive(Debug, Clone, PartialEq)]
pub struct Installment {
    pub value: InstallmentValue,
    pub value_amount: Decimal,
    pub days: i32,
    /// Due at the end of the month the days lead to, then `days_after_end_of_month` more.
    pub end_of_month: bool,
    pub days_after_end_of_month: i32,
}

/// The last day of the month `date` is in.
pub fn end_of_month(date: NaiveDate) -> NaiveDate {
    let first = date.with_day(1).expect("every month has a first day");
    first + Months::new(1) - TimeDelta::days(1)
}

/// The date an installment falls due, for an invoice of `date`.
pub fn due_date(date: NaiveDate, installment: &Installment) -> NaiveDate {
    let due = date + TimeDelta::days(installment.days as i64);
    if installment.end_of_month {
        end_of_month(due) + TimeDelta::days(installment.days_after_end_of_month as i64)
    } else {
        due
    }
}

/// The installments of `total`, due from `date`: `(due date, amount)`, in the order given.
///
/// Errs unless the term ends with a single balance and its percentages stay within 100 %.
/// No term at all is one installment, due on the invoice's date.
pub fn installments(
    total: Decimal,
    date: NaiveDate,
    term: &[Installment],
    rounding: Decimal,
) -> Result<Vec<(NaiveDate, Decimal)>, String> {
    if term.is_empty() {
        return Ok(vec![(date, total)]);
    }
    check(term)?;
    let mut left = total;
    let mut result = Vec::new();
    for installment in term {
        let amount = match installment.value {
            InstallmentValue::Percent => money::round(
                total * installment.value_amount / Decimal::ONE_HUNDRED,
                rounding,
            ),
            InstallmentValue::Balance => left,
        };
        left -= amount;
        result.push((due_date(date, installment), amount));
    }
    Ok(result)
}

/// A term ends with its only balance line, and its percentages add up to 100 % at most.
pub fn check(term: &[Installment]) -> Result<(), String> {
    let balances = term
        .iter()
        .filter(|installment| installment.value == InstallmentValue::Balance)
        .count();
    if balances != 1
        || term.last().map(|installment| installment.value) != Some(InstallmentValue::Balance)
    {
        return Err("A payment term ends with one balance line, and has no other".to_string());
    }
    let percent: Decimal = term
        .iter()
        .filter(|installment| installment.value == InstallmentValue::Percent)
        .map(|installment| installment.value_amount)
        .sum();
    if percent > Decimal::ONE_HUNDRED
        || term
            .iter()
            .any(|installment| installment.value_amount < Decimal::ZERO)
    {
        return Err("The percentages of a payment term add up to 100 % at most".to_string());
    }
    if term
        .iter()
        .any(|installment| installment.days < 0 || installment.days_after_end_of_month < 0)
    {
        return Err("A payment term counts days forward".to_string());
    }
    Ok(())
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

    fn balance(days: i32) -> Installment {
        Installment {
            value: InstallmentValue::Balance,
            value_amount: Decimal::ZERO,
            days,
            end_of_month: false,
            days_after_end_of_month: 0,
        }
    }

    fn percent(value: &str, days: i32) -> Installment {
        Installment {
            value: InstallmentValue::Percent,
            value_amount: d(value),
            days,
            end_of_month: false,
            days_after_end_of_month: 0,
        }
    }

    #[test]
    fn test_immediate_and_thirty_days() {
        assert_eq!(
            installments(d("121"), date("2026-01-15"), &[], d("0.01")),
            Ok(vec![(date("2026-01-15"), d("121"))])
        );
        assert_eq!(
            installments(d("121"), date("2026-01-15"), &[balance(30)], d("0.01")),
            Ok(vec![(date("2026-02-14"), d("121"))])
        );
    }

    #[test]
    fn test_thirty_days_end_of_month() {
        let term = [Installment {
            end_of_month: true,
            ..balance(30)
        }];
        assert_eq!(
            installments(d("100"), date("2026-01-15"), &term, d("0.01")),
            Ok(vec![(date("2026-02-28"), d("100"))])
        );
        assert_eq!(due_date(date("2028-01-31"), &term[0]), date("2028-03-31"));
        let term = [Installment {
            end_of_month: true,
            days_after_end_of_month: 10,
            ..balance(0)
        }];
        assert_eq!(due_date(date("2026-12-05"), &term[0]), date("2027-01-10"));
    }

    #[test]
    fn test_percentages_then_balance_add_up() {
        // 30 % now, the rest at 60 days: 100.01 → 30.00 + 70.01.
        let term = [percent("30", 0), balance(60)];
        let result = installments(d("100.01"), date("2026-03-01"), &term, d("0.01")).expect("due");
        assert_eq!(
            result,
            vec![
                (date("2026-03-01"), d("30")),
                (date("2026-04-30"), d("70.01"))
            ]
        );
        // Thirds of 100: 33.33 + 33.33 + 33.34.
        let term = [percent("33.3333", 30), percent("33.3333", 60), balance(90)];
        let result = installments(d("100"), date("2026-03-01"), &term, d("0.01")).expect("due");
        let amounts: Vec<Decimal> = result.iter().map(|(_, amount)| *amount).collect();
        assert_eq!(amounts, vec![d("33.33"), d("33.33"), d("33.34")]);
    }

    #[test]
    fn test_a_refund_falls_due_alike() {
        let term = [percent("50", 0), balance(30)];
        let result = installments(d("-99.99"), date("2026-03-01"), &term, d("0.01")).expect("due");
        let amounts: Vec<Decimal> = result.iter().map(|(_, amount)| *amount).collect();
        assert_eq!(amounts, vec![d("-50"), d("-49.99")]);
        assert_eq!(amounts.iter().copied().sum::<Decimal>(), d("-99.99"));
    }

    #[test]
    fn test_a_term_must_end_with_its_balance() {
        let today = date("2026-03-01");
        assert!(installments(d("1"), today, &[percent("100", 0)], d("0.01")).is_err());
        assert!(installments(d("1"), today, &[balance(0), percent("10", 0)], d("0.01")).is_err());
        assert!(installments(d("1"), today, &[balance(0), balance(10)], d("0.01")).is_err());
        assert!(
            installments(
                d("1"),
                today,
                &[percent("60", 0), percent("50", 0), balance(0)],
                d("0.01")
            )
            .is_err()
        );
        assert!(installments(d("1"), today, &[balance(-1)], d("0.01")).is_err());
    }
}
