use crate::models::account_invoice_line::AccountInvoiceLinePurchase;
use crate::models::purchase_order_line::PurchaseOrderLine;
use account::models::{AccountInvoiceLine, AccountMove};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, SingleId};

/// A vendor credit note of an order's bill counts against the order.
#[derive(Model)]
#[erp(id = "account_move", methods)]
#[erp(derived_model = "account::models")]
#[allow(dead_code)]
pub struct AccountMovePurchase<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl AccountMovePurchase<MultipleIds> {
    /// Each line of the credit note bills back the order lines its bill's line billed.
    pub fn link_reversal(&self, env: &mut Environment, reversal: u32, sup: Super) -> Result<()> {
        sup.call(env)?;
        let env = &mut *env.sudo();
        let reversal: AccountMove<SingleId> = env.get_record(reversal.into());
        let copies: AccountInvoiceLine<MultipleIds> = reversal.get_invoice_lines(env)?;
        for origin in self {
            let origin: AccountMove<SingleId> = origin.as_model();
            let originals: AccountInvoiceLine<MultipleIds> = origin.get_invoice_lines(env)?;
            for (original, copy) in originals.into_iter().zip(copies.clone()) {
                let original: AccountInvoiceLinePurchase<SingleId> = original.as_model();
                let order_lines: PurchaseOrderLine<MultipleIds> =
                    original.get_purchase_lines(env)?;
                if order_lines.is_empty() {
                    continue;
                }
                let copy: AccountInvoiceLinePurchase<SingleId> = copy.as_model();
                copy.set_purchase_lines(&order_lines, env)?;
            }
        }
        Ok(())
    }
}
