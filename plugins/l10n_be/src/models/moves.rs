use crate::structured::structured_communication;
use account::models::{Move, MoveType};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, SingleId};

/// Customer invoices are paid with a structured communication.
#[derive(Model)]
#[erp(id = "account_move", methods)]
#[erp(derived_model = "account::models")]
#[allow(dead_code)]
pub struct MoveBe<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl MoveBe<MultipleIds> {
    /// A customer invoice or credit note gets the structured communication of its id; other
    /// entries what accounting gives them.
    pub fn compute_payment_reference(&self, env: &mut Environment, sup: Super) -> Result<()> {
        let mut others = Vec::new();
        for entry in self {
            let entry: Move<SingleId> = env.get_record(entry.get_id().into());
            let customer = matches!(
                *entry.get_move_type(env)?,
                MoveType::OutInvoice | MoveType::OutRefund
            );
            if customer && entry.get_payment_reference(env)?.is_none() {
                let reference = structured_communication(u64::from(entry.get_id()));
                entry.set_payment_reference(Some(reference), env)?;
            } else {
                others.push(entry.get_id());
            }
        }
        sup.call_on(others, env)
    }
}
