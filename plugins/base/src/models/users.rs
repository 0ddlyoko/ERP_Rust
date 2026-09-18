use crate::models::BaseGroup;
use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference};

/// Someone who can log in.
#[derive(Model)]
#[erp(id = "users")]
#[allow(dead_code)]
pub struct Users<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    login: String,
    /// Argon2 hash. The clear password is never stored, and never recoverable.
    #[erp(default = "")]
    password: String,
    #[erp(default = "")]
    name: String,
    #[erp(default = true)]
    active: bool,
    #[erp(relation = "user_group_rel")]
    groups: Reference<BaseGroup, MultipleIds>,
}
