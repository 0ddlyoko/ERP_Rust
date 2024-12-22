use crate::models::lang::Baselang;
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
    pub lang: Reference<Baselang>,
    // TODO Link to country
    // TODO Link to another contact (company)
}
