use crate::models::account_invoice_line::AccountInvoiceLineSale;
use crate::models::sale_order_line::SaleOrderLine;
use account::models::{AccountInvoiceLine, AccountMove};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, SingleId};

/// A credit note of an order's invoice counts against the order.
#[derive(Model)]
#[erp(id = "account_move", methods)]
#[erp(derived_model = "account::models")]
#[allow(dead_code)]
pub struct AccountMoveSale<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl AccountMoveSale<MultipleIds> {
    /// Each line of the credit note invoices back the order lines its invoice's line invoiced:
    /// the lines were copied in order.
    pub fn link_reversal(&self, env: &mut Environment, reversal: u32, sup: Super) -> Result<()> {
        sup.call(env)?;
        let env = &mut *env.sudo();
        let reversal: AccountMove<SingleId> = env.get_record(reversal.into());
        let copies: AccountInvoiceLine<MultipleIds> = reversal.get_invoice_lines(env)?;
        for origin in self {
            let origin: AccountMove<SingleId> = origin.as_model();
            let originals: AccountInvoiceLine<MultipleIds> = origin.get_invoice_lines(env)?;
            for (original, copy) in originals.into_iter().zip(copies.clone()) {
                let original: AccountInvoiceLineSale<SingleId> = original.as_model();
                let order_lines: SaleOrderLine<MultipleIds> = original.get_sale_lines(env)?;
                if order_lines.is_empty() {
                    continue;
                }
                let copy: AccountInvoiceLineSale<SingleId> = copy.as_model();
                copy.set_sale_lines(&order_lines, env)?;
            }
        }
        Ok(())
    }
}
