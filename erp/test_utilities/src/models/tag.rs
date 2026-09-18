use crate::models::BaseInvoice;
use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference};

/// Both sides of a many2many name the same relation table.
#[derive(Model)]
#[erp(id = "tag")]
#[allow(dead_code)]
pub struct Tag<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    name: String,
    #[erp(relation = "invoice_tag_rel")]
    invoices: Reference<BaseInvoice, MultipleIds>,
}
