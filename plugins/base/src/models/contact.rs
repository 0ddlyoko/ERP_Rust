use crate::models::lang::BaseLang;
use code_gen::Model;
use erp::field::Reference;

#[derive(Model)]
#[erp(table_name="contact")]
pub struct Contact {
    pub id: u32,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub lang: Reference<BaseLang>,
    // TODO Link to country
    // TODO Link to another contact (company)
}

#[derive(Model)]
#[erp(table_name="contact")]
#[erp(derived_model = "crate::models::contact")]
pub struct Contact2 {
    id: u32,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
    lang: Reference<BaseLang>,
}
