use crate::models::sale_order_line::BaseSaleOrderLine;
use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference};

/// The order lines an invoice line invoices.
#[derive(Model)]
#[erp(id = "account_invoice_line")]
#[erp(derived_model = "account::models")]
#[allow(dead_code)]
pub struct AccountInvoiceLineSale<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Order lines", relation = "sale_order_line_invoice_rel")]
    sale_lines: Reference<BaseSaleOrderLine, MultipleIds>,
}
