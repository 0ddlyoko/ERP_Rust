use code_gen::Model;
use erp::field::{IdMode, Reference, SingleId};
use crate::models::contact::BaseContact;

#[derive(Model)]
#[erp(table_name="company")]
pub struct Company<Mode: IdMode> {
    id: Mode,
    name: String,
    contact: Reference<BaseContact, SingleId>,
}
