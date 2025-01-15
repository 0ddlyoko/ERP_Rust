use crate::models::lang::Baselang;
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
    lang: Reference<Baselang>,
    // TODO Link to country
    // TODO Link to another contact (company)
}
