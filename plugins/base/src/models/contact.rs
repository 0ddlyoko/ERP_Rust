use crate::models::country::BaseCountry;
use crate::models::lang::BaseLang;
use code_gen::Model;
use erp::field::Reference;

#[derive(Model)]
#[erp(table_name="contact")]
pub struct Contact {
    pub id: u32,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
    lang: Reference<BaseLang>,
    country: Reference<BaseCountry>,
    parent: Reference<BaseContact>,
}
