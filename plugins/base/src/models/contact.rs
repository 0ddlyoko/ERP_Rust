use crate::models::country::BaseCountry;
use crate::models::lang::BaseLang;
use code_gen::Model;
use erp::field::{Reference, SingleId};

#[derive(Model)]
#[erp(table_name="contact")]
pub struct Contact {
    id: u32,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
    lang: Reference<BaseLang, SingleId>,
    country: Reference<BaseCountry, SingleId>,
    parent: Reference<BaseContact, SingleId>,
}
