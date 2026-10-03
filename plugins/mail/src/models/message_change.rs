use crate::models::BaseMessage;
use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};

/// One tracked field a message notes the change of, as it read then: its label, its values
/// before and after as a person reads them, and as they are stored — a selection's key, a
/// record's id — for a client to match against.
#[derive(Model)]
#[erp(id = "message_change")]
#[allow(dead_code)]
pub struct MessageChange<Mode: IdMode> {
    pub id: Mode,
    #[erp(ondelete = "cascade")]
    message: Reference<BaseMessage, SingleId>,
    #[erp(default = "")]
    field: String,
    #[erp(default = "")]
    label: String,
    old: Option<String>,
    new: Option<String>,
    old_value: Option<String>,
    new_value: Option<String>,
}
