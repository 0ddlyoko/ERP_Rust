//! What Belgian books always hold to, beyond accounting's own invariants: every VAT booked is
//! at a Belgian rate — 21, 12, 6 or 0 % — and the return's 71 or 72 is what its grids leave.

use crate::models::grid_amounts;
use account::models::{AccountMoveLine, AccountTax, LineKind, MoveState};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, MultipleIds, NaiveDate, SingleId};
use erp_search_code_gen::make_domain;

const BELGIAN_RATES: [u32; 4] = [21, 12, 6, 0];

/// Every tax item posted is at a Belgian rate, and accounting's invariants hold.
pub fn check_books(env: &mut Environment) -> Result<()> {
    account::invariants::check_books(env)?;
    let env = &mut *env.sudo();
    let items: AccountMoveLine<MultipleIds> = env.search(&make_domain!([
        ("parent_state", "=", MoveState::Posted),
        ("display_type", "=", LineKind::Tax)
    ]))?;
    for item in &items {
        let tax: AccountTax<SingleId> = item.get_tax_line(env)?;
        let rate = *tax.get_amount(env)?;
        if !BELGIAN_RATES
            .iter()
            .any(|belgian| rate == Decimal::from(*belgian))
        {
            return Err(format!(
                "The tax {} is at {rate} %, no Belgian rate",
                tax.get_name(env)?
            )
            .into());
        }
    }
    Ok(())
}

/// The return of a period settles to what its grids leave: 71 = due less deductible when
/// positive, 72 the other way round.
pub fn check_return(env: &mut Environment, from: NaiveDate, to: NaiveDate) -> Result<()> {
    let grids = grid_amounts(env, from, to)?;
    let grid = |code: &str| grids.get(code).copied().unwrap_or_default();
    let due = grid("54") + grid("55") + grid("56") + grid("57") + grid("61") + grid("63");
    let deductible = grid("59") + grid("62") + grid("64");
    if grid("71") - grid("72") != due - deductible {
        return Err(format!(
            "The return does not settle: 71 {} and 72 {} for {due} due and {deductible} deductible",
            grid("71"),
            grid("72")
        )
        .into());
    }
    Ok(())
}
