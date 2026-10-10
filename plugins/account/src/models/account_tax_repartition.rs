use crate::models::account::BaseAccount;
use crate::models::account_tax::BaseAccountTax;
use crate::models::account_tax_tag::BaseAccountTaxTag;
use code_gen::{Model, selection};
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};

#[selection]
pub enum TaxDocument {
    #[default]
    #[selection(label = "Invoices")]
    Invoice,
    #[selection(label = "Credit notes")]
    Refund,
}

/// Where a share of a tax goes, on invoices or on credit notes.
#[derive(Model)]
#[erp(id = "account_tax_repartition", order = "sequence, id")]
#[allow(dead_code)]
pub struct AccountTaxRepartition<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    tax: Reference<BaseAccountTax, SingleId>,
    #[erp(label = "Applies to")]
    document: TaxDocument,
    #[erp(
        label = "%",
        default = 100.0,
        description = "The share of the tax: 100, or -100 for the VAT owed by a reverse charge"
    )]
    factor: Decimal,
    #[erp(ondelete = "restrict")]
    account: Reference<BaseAccount, SingleId>,
    #[erp(label = "Tax grids", relation = "account_tax_repartition_tag_rel")]
    tags: Reference<BaseAccountTaxTag, MultipleIds>,
    #[erp(default = 10)]
    sequence: i32,
}
