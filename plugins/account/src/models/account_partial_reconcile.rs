use crate::models::account_full_reconcile::BaseAccountFullReconcile;
use crate::models::account_move_line::{BaseAccountMoveLine, MoveLine};
use code_gen::Model;
use currency::models::Currency;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, Reference, SingleId};

/// A debit and a credit settling each other for an amount: an invoice and part of its payment.
#[derive(Model)]
#[erp(id = "account_partial_reconcile")]
#[allow(dead_code)]
pub struct PartialReconcile<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    debit_line: Reference<BaseAccountMoveLine, SingleId>,
    #[erp(required, ondelete = "cascade")]
    credit_line: Reference<BaseAccountMoveLine, SingleId>,
    #[erp(description = "In the company's currency")]
    amount: Decimal,
    #[erp(label = "Amount of the debit in its currency", default = 0.0)]
    debit_amount_currency: Decimal,
    #[erp(label = "Amount of the credit in its currency", default = 0.0)]
    credit_amount_currency: Decimal,
    #[erp(ondelete = "set_null")]
    full_reconcile: Reference<BaseAccountFullReconcile, SingleId>,
}

/// `matched` of an item worth `amount` in one currency and `other` in the other, in that other,
/// rounded to that other currency's `rounding`.
pub(crate) fn prorata(
    other: Decimal,
    amount: Decimal,
    matched: Decimal,
    rounding: Decimal,
) -> Decimal {
    if amount.is_zero() {
        return Decimal::ZERO;
    }
    currency::money::round(other * matched / amount, rounding)
}

/// The rounding of the currency a journal item is in: its own, else the company's.
pub(crate) fn currency_rounding(env: &mut Environment, line: u32) -> Result<Decimal> {
    let env = &mut *env.sudo();
    let line: MoveLine<SingleId> = env.get_record(line.into());
    let currency: Currency<SingleId> = line.get_currency(env)?;
    let currency = if currency.is_empty() {
        Currency::of_company(env)?
    } else {
        currency
    };
    Ok(*currency.get_rounding(env)?)
}
