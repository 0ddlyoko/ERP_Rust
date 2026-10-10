use base::models::BaseContact;
use code_gen::Model;
use erp::types::field::{Decimal, IdMode, Reference, SingleId};
use product::models::BaseProduct;

/// The price a vendor asks for a product, from a quantity on, and how long it takes to come.
#[derive(Model)]
#[erp(id = "product_supplierinfo", order = "sequence, id")]
#[allow(dead_code)]
pub struct SupplierInfo<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Vendor", required, ondelete = "cascade")]
    partner: Reference<BaseContact, SingleId>,
    #[erp(required, ondelete = "cascade")]
    product: Reference<BaseProduct, SingleId>,
    #[erp(label = "Vendor product code")]
    product_code: Option<String>,
    #[erp(label = "Minimum quantity", default = 0.0)]
    min_quantity: Decimal,
    #[erp(default = 0.0)]
    price: Decimal,
    #[erp(label = "Delivery lead time (days)", default = 1)]
    delay: i32,
    #[erp(default = 10)]
    sequence: i32,
}
