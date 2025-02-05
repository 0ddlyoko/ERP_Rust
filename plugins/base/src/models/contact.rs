use crate::models::country::BaseCountry;
use crate::models::lang::BaseLang;
use code_gen::Model;
use erp::field::{IdMode, MultipleIds, Reference, SingleId};

#[derive(Model)]
#[erp(table_name="contact")]
pub struct Contact<Mode: IdMode> {
    id: Mode,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
    lang: Reference<BaseLang, SingleId>,
    country: Reference<BaseCountry, SingleId>,
    parent: Reference<BaseContact, SingleId>,
    // TODO MultipleIds should not exist without SingleId ref
    // childrens: Reference<BaseContact, MultipleIds>,
}
