use crate::models::BaseMessage;
use base::models::BaseContact;
use code_gen::{Model, selection};
use erp::types::field::{IdMode, Reference, SingleId, Timestamp};

/// Where a mail stands: waiting to be sent, sent, or given up on.
#[selection]
pub enum MailState {
    #[default]
    #[selection(label = "Outgoing")]
    Outgoing,
    #[selection(label = "Sent")]
    Sent,
    #[selection(label = "Failed")]
    Failed,
}

/// A mail sent, or to send, to someone a message was said to outside the application: its
/// address as it was then, what it says, and where it stands — with why, when it failed.
#[derive(Model)]
#[erp(id = "mail", order = "id desc")]
#[allow(dead_code)]
pub struct Mail<Mode: IdMode> {
    pub id: Mode,
    #[erp(ondelete = "cascade", index)]
    message: Reference<BaseMessage, SingleId>,
    #[erp(label = "Recipient", ondelete = "set_null")]
    recipient: Reference<BaseContact, SingleId>,
    #[erp(label = "Address")]
    email: Option<String>,
    subject: Option<String>,
    body: Option<String>,
    #[erp(label = "Status", index, readonly)]
    state: MailState,
    error: Option<String>,
    #[erp(label = "Sent on")]
    sent_at: Option<Timestamp>,
}
