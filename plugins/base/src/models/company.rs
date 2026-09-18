use crate::models::contact::BaseContact;
use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};

#[derive(Model)]
#[erp(id = "company")]
#[allow(dead_code)]
pub struct Company<Mode: IdMode> {
    id: Mode,
    name: String,
    contact: Reference<BaseContact, SingleId>,
}
