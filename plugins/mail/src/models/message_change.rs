use crate::models::BaseMessage;
use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};

/// One tracked field a message notes the change of, as it read then: its label, and its values
/// before and after as text.
#[derive(Model)]
#[erp(id = "message_change")]
#[allow(dead_code)]
pub struct MessageChange<Mode: IdMode> {
    pub id: Mode,
    message: Reference<BaseMessage, SingleId>,
    #[erp(default = "")]
    field: String,
    #[erp(default = "")]
    label: String,
    old: Option<String>,
    new: Option<String>,
}
