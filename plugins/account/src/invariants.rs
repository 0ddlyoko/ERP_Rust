//! What the books always hold to, checked from the records: every posted entry balances, so the
//! trial balance does, and every tax booked is the tax its invoice's lines make.
//!
//! Tests run these after every scenario booking entries; a broken invariant names the entry.

use crate::models::{InvoiceLine, LineKind, Move, MoveLine, MoveState, Tax, TaxDocument};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::Selection;
use erp::types::field::{Decimal, MultipleIds, SingleId};
use erp_search_code_gen::make_domain;
use std::collections::BTreeMap;

/// Every posted entry balances, and so do all of them together.
pub fn check_balanced(env: &mut Environment) -> Result<()> {
    let env = &mut *env.sudo();
    let entries: Move<MultipleIds> = env.search(&make_domain!([(
        "state",
        "=",
        MoveState::Posted.key().as_str()
    )]))?;
    let mut debit_all = Decimal::ZERO;
    let mut credit_all = Decimal::ZERO;
    for entry in &entries {
        let (debit, credit) = entry.totals(env)?;
        if debit != credit {
            return Err(format!(
                "{} does not balance: {debit} debited, {credit} credited",
                entry.get_name(env)?
            )
            .into());
        }
        debit_all += debit;
        credit_all += credit;
    }
    let lines: MoveLine<MultipleIds> = env.search(&make_domain!([(
        "parent_state",
        "=",
        MoveState::Posted.key().as_str()
    )]))?;
    let mut balance = Decimal::ZERO;
    for line in &lines {
        balance += *line.get_balance(env)?;
    }
    if debit_all != credit_all || !balance.is_zero() {
        return Err(format!(
            "The trial balance does not balance: {debit_all} debited, {credit_all} credited"
        )
        .into());
    }
    Ok(())
}

/// Every tax booked on a posted invoice is what its lines make at the tax's rate: per tax, the
/// items in the invoice's currency add up to the shares the engine computes.
pub fn check_invoice_taxes(env: &mut Environment) -> Result<()> {
    let env = &mut *env.sudo();
    let entries: Move<MultipleIds> = env.search(&make_domain!([(
        "state",
        "=",
        MoveState::Posted.key().as_str()
    )]))?;
    for entry in &entries {
        let move_type = *entry.get_move_type(env)?;
        if !move_type.is_invoice() {
            continue;
        }
        let document = if move_type.is_refund() {
            TaxDocument::Refund
        } else {
            TaxDocument::Invoice
        };
        let mut expected: BTreeMap<u32, Decimal> = BTreeMap::new();
        let lines: InvoiceLine<MultipleIds> = entry.get_invoice_lines(env)?;
        for line in &lines {
            for tax in line.taxed(env, document)?.taxes {
                let shares: Decimal = tax.shares.iter().map(|(_, share)| share.abs()).sum();
                *expected.entry(tax.tax).or_default() += shares;
            }
        }
        let mut booked: BTreeMap<u32, Decimal> = BTreeMap::new();
        let items: MoveLine<MultipleIds> = entry.get_lines(env)?;
        for item in &items {
            if matches!(*item.get_display_type(env)?, LineKind::Tax) {
                let tax: Tax<SingleId> = item.get_tax_line(env)?;
                *booked.entry(tax.get_id()).or_default() += item.get_amount_currency(env)?.abs();
            }
        }
        expected.retain(|_, amount| !amount.is_zero());
        if expected != booked {
            return Err(format!(
                "The taxes of {} are not those of its lines: {booked:?} booked, {expected:?} computed",
                entry.get_name(env)?
            )
            .into());
        }
    }
    Ok(())
}

/// All of the above.
pub fn check_books(env: &mut Environment) -> Result<()> {
    check_balanced(env)?;
    check_invoice_taxes(env)
}
