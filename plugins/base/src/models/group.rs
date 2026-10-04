use crate::models::{BaseUsers, Users};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference};

/// A set of users, which access rights will later be granted to.
#[derive(Model)]
#[erp(id = "group", methods)]
#[allow(dead_code)]
pub struct Group<Mode: IdMode> {
    pub id: Mode,
    name: String,
    #[erp(relation = "user_group_rel")]
    users: Reference<BaseUsers, MultipleIds>,
    #[erp(label = "Number of users", compute = "compute_user_count", depends = ["users"])]
    user_count: i32,
}

#[erp_methods]
impl Group<MultipleIds> {
    pub fn compute_user_count(&self, env: &mut Environment) -> Result<()> {
        for group in self {
            let users: Users<MultipleIds> = group.get_users(env)?;
            group.set_user_count(i32::try_from(users.get_ids().len())?, env)?;
        }
        Ok(())
    }
}
