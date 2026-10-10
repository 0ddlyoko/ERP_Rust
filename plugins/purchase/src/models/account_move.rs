use crate::models::account_invoice_line::InvoiceLinePurchase;
use crate::models::purchase_order_line::PurchaseOrderLine;
use account::models::{InvoiceLine, Move};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, SingleId};

/// A vendor credit note of an order's bill counts against the order.
#[derive(Model)]
#[erp(id = "account_move", methods)]
#[erp(derived_model = "account::models")]
#[allow(dead_code)]
pub struct MovePurchase<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl MovePurchase<MultipleIds> {
    /// Each line of the credit note bills back the order lines its bill's line billed.
    pub fn link_reversal(&self, env: &mut Environment, reversal: u32, sup: Super) -> Result<()> {
        sup.call(env)?;
        let env = &mut *env.sudo();
        let reversal: Move<SingleId> = env.get_record(reversal.into());
        let copies: InvoiceLine<MultipleIds> = reversal.get_invoice_lines(env)?;
        for origin in self {
            let origin: Move<SingleId> = origin.as_model();
            let originals: InvoiceLine<MultipleIds> = origin.get_invoice_lines(env)?;
            for (original, copy) in originals.into_iter().zip(copies.clone()) {
                let original: InvoiceLinePurchase<SingleId> = original.as_model();
                let order_lines: PurchaseOrderLine<MultipleIds> =
                    original.get_purchase_lines(env)?;
                if order_lines.is_empty() {
                    continue;
                }
                let copy: InvoiceLinePurchase<SingleId> = copy.as_model();
                copy.set_purchase_lines(&order_lines, env)?;
            }
        }
        Ok(())
    }
}
