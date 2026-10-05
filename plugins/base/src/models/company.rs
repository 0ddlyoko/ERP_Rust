use crate::models::contact::BaseContact;
use code_gen::Model;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp_search::SearchType;

/// The business running this database: its name, and the contact holding its address and VAT.
#[derive(Model)]
#[erp(id = "company")]
#[allow(dead_code)]
pub struct Company<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(required, ondelete = "restrict")]
    contact: Reference<BaseContact, SingleId>,
}

impl Company<SingleId> {
    /// The company documents are made for: the first one, the seeded `base.main_company` unless
    /// it was removed. Empty on a database that has none.
    ///
    /// As sudo: which company is running the database is no secret to anyone working in it.
    pub fn current(env: &mut Environment) -> Result<Company<SingleId>> {
        let env = &mut *env.sudo();
        let companies: Company<MultipleIds> = env.search(&SearchType::Nothing)?;
        Ok(match companies.get_ids_ref().iter().min() {
            Some(id) => env.get_record((*id).into()),
            None => env.get_record(SingleId::empty()),
        })
    }
}
