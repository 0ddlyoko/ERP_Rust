use code_gen::Model;
use erp::field::{Reference, SingleId};
use crate::models::contact::BaseContact;

#[derive(Model)]
#[erp(table_name="company")]
pub struct Company {
    id: u32,
    name: String,
    contact: Reference<BaseContact, SingleId>,
}
