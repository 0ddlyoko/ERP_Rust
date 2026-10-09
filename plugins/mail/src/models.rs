pub(crate) mod follower;
mod mail;
mod message;
mod message_change;
mod notification;
mod subtype;

pub use follower::{BaseFollower, Follower};
pub use mail::{BaseMail, Mail, MailState};
pub use message::{BaseMessage, Message, MessageKind, forget_deleted, note_changes, note_creation};
pub use message_change::{BaseMessageChange, MessageChange};
pub use notification::{BaseNotification, Notification, NotificationReason};
pub use subtype::{BaseMessageSubtype, MessageSubtype};
