use crate::models::purchase_order_line::BasePurchaseOrderLine;
use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference};

/// The purchase order lines a vendor bill line bills.
#[derive(Model)]
#[erp(id = "account_invoice_line")]
#[erp(derived_model = "account::models")]
#[allow(dead_code)]
pub struct InvoiceLinePurchase<Mode: IdMode> {
    id: Mode,
    #[erp(
        label = "Purchase order lines",
        relation = "purchase_order_line_invoice_rel"
    )]
    purchase_lines: Reference<BasePurchaseOrderLine, MultipleIds>,
}
