mod message;
mod message_change;

pub use message::{BaseMessage, Message, MessageKind, forget_deleted, note_changes};
pub use message_change::{BaseMessageChange, MessageChange};
