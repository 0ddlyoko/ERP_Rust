use crate::models::BaseUsers;
use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference};

/// A set of users, which access rights will later be granted to.
#[derive(Model)]
#[erp(id = "group")]
#[allow(dead_code)]
pub struct Group<Mode: IdMode> {
    pub id: Mode,
    name: String,
    #[erp(relation = "user_group_rel")]
    users: Reference<BaseUsers, MultipleIds>,
}
