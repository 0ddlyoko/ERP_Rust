use crate::models::{BaseInvoice, BaseSaleOrder};
use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference};

/// Both sides of a many2many name the same relation table.
#[derive(Model)]
#[erp(id = "tag")]
#[allow(dead_code)]
pub struct Tag<Mode: IdMode> {
    pub id: Mode,
    name: String,
    /// Named after the structural element on purpose: it pins down that `<field>` with no `name`
    /// attribute reaches the field called `field`, and not the long form.
    field: Option<String>,
    #[erp(relation = "invoice_tag_rel")]
    invoices: Reference<BaseInvoice, MultipleIds>,
    #[erp(relation = "sale_order_tag_rel")]
    orders: Reference<BaseSaleOrder, MultipleIds>,
}
