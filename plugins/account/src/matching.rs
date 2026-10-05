//! Matching debits with credits, apart from the records: which amount of which debit settles
//! which credit.

use erp::types::field::Decimal;

/// An open amount of a line to match: its id and what is left of it, positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Open {
    pub id: u32,
    pub residual: Decimal,
}

/// The pairs `(debit, credit, amount)` settling `debits` against `credits`, oldest first: each
/// debit takes from the credits in order until one of the two is exhausted.
///
/// What cannot be matched stays open; nothing is matched twice.
pub fn match_lines(debits: &[Open], credits: &[Open]) -> Vec<(u32, u32, Decimal)> {
    let mut debits: Vec<Open> = debits
        .iter()
        .copied()
        .filter(|open| open.residual > Decimal::ZERO)
        .collect();
    let mut credits: Vec<Open> = credits
        .iter()
        .copied()
        .filter(|open| open.residual > Decimal::ZERO)
        .collect();
    let mut pairs = Vec::new();
    let (mut d, mut c) = (0, 0);
    while d < debits.len() && c < credits.len() {
        let amount = debits[d].residual.min(credits[c].residual);
        pairs.push((debits[d].id, credits[c].id, amount));
        debits[d].residual -= amount;
        credits[c].residual -= amount;
        if debits[d].residual.is_zero() {
            d += 1;
        }
        if credits[c].residual.is_zero() {
            c += 1;
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn d(value: &str) -> Decimal {
        Decimal::from_str(value).expect("a decimal")
    }

    fn open(id: u32, residual: &str) -> Open {
        Open {
            id,
            residual: d(residual),
        }
    }

    #[test]
    fn test_one_invoice_one_payment() {
        assert_eq!(
            match_lines(&[open(1, "121")], &[open(2, "121")]),
            vec![(1, 2, d("121"))]
        );
    }

    #[test]
    fn test_partial_payment_leaves_the_rest_open() {
        assert_eq!(
            match_lines(&[open(1, "121")], &[open(2, "100")]),
            vec![(1, 2, d("100"))]
        );
    }

    #[test]
    fn test_one_payment_for_two_invoices() {
        assert_eq!(
            match_lines(&[open(1, "100"), open(3, "50")], &[open(2, "120")]),
            vec![(1, 2, d("100")), (3, 2, d("20"))]
        );
    }

    #[test]
    fn test_two_payments_for_one_invoice() {
        assert_eq!(
            match_lines(
                &[open(1, "100")],
                &[open(2, "30"), open(4, "70"), open(5, "10")]
            ),
            vec![(1, 2, d("30")), (1, 4, d("70"))]
        );
    }

    #[test]
    fn test_nothing_to_match() {
        assert!(match_lines(&[], &[open(2, "1")]).is_empty());
        assert!(match_lines(&[open(1, "0")], &[open(2, "1")]).is_empty());
    }
}
