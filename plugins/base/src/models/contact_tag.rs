use code_gen::Model;
use erp::types::field::IdMode;

/// A label put on contacts to sort them: `Supplier`, `VIP`, `Prospect`.
#[derive(Model)]
#[erp(id = "contact_tag", order = "name, id")]
#[allow(dead_code)]
pub struct ContactTag<Mode: IdMode> {
    id: Mode,
    name: String,
}
