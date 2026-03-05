use crate::models::contact::BaseContact;
use code_gen::Model;
use erp::field::Reference;
use erp::types::field::{IdMode, SingleId};

#[derive(Model)]
#[erp(table_name = "company")]
#[allow(dead_code)]
pub struct Company<Mode: IdMode> {
    id: Mode,
    name: String,
    contact: Reference<BaseContact, SingleId>,
}
