mod message;
mod message_change;

pub use message::{BaseMessage, Message, MessageKind, forget_deleted, note_changes, note_creation};
pub use message_change::{BaseMessageChange, MessageChange};
