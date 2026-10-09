use crate::models::{BaseMessage, Message};
use base::models::BaseUsers;
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::serde_json::{Value, json};
use erp::types::field::{IdMode, MultipleIds, Reference, Selection, SingleId};
use erp::types::model::MapOfFields;
use erp_search::SearchOptions;
use erp_search_code_gen::make_domain;

/// Why a user is told of a message: they follow its record, or they are mentioned in it.
#[selection]
pub enum NotificationReason {
    #[default]
    #[selection(label = "Followed")]
    Follower,
    #[selection(label = "Mentioned")]
    Mention,
}

/// A message a user is told of, in their inbox until read.
#[derive(Model)]
#[erp(id = "notification", order = "id desc", methods)]
#[allow(dead_code)]
pub struct Notification<Mode: IdMode> {
    pub id: Mode,
    #[erp(required, ondelete = "cascade")]
    message: Reference<BaseMessage, SingleId>,
    #[erp(required, ondelete = "cascade", index)]
    user: Reference<BaseUsers, SingleId>,
    #[erp(index)]
    read: bool,
    reason: NotificationReason,
}

#[erp_methods]
impl Notification<MultipleIds> {
    /// The user's inbox, newest first: each message they were told of, why, whether they read it,
    /// and the record it is about, named.
    #[erp(rpc)]
    pub fn inbox(&self, env: &mut Environment, limit: Option<usize>) -> Result<Value> {
        let _ = self;
        let Some(uid) = env.uid() else {
            return Ok(json!([]));
        };
        let env = &mut *env.sudo();
        let options = SearchOptions::new().with_limit(limit.unwrap_or(30));
        let mine: Notification<MultipleIds> =
            env.search_with(&make_domain!([("user", "=", uid)]), &options)?;
        let mut inbox = Vec::new();
        for notification in mine {
            let message: Message<SingleId> = notification.get_message(env)?;
            let model = message.get_model(env)?.clone();
            let record = u32::try_from(*message.get_record(env)?)?;
            let name = env.names(&model, &[record])?.remove(&record);
            inbox.push(json!({
                "id": notification.get_id(),
                "read": notification.get_read(env)?,
                "reason": notification.get_reason(env)?.key().as_str(),
                "model": model,
                "record": record,
                "record_name": name,
                "message": message.describe(env)?,
            }));
        }
        Ok(Value::Array(inbox))
    }

    /// How many messages the user was told of and has not read.
    #[erp(rpc)]
    pub fn unread(&self, env: &mut Environment) -> Result<u64> {
        let _ = self;
        let Some(uid) = env.uid() else {
            return Ok(0);
        };
        let domain = make_domain!([("user", "=", uid), ("read", "=", false)]);
        Ok(env.sudo().count("notification", &domain)? as u64)
    }

    /// Mark the user's notifications read: these, or all of them when none is named.
    #[erp(rpc)]
    pub fn mark_read(&self, env: &mut Environment, notifications: Vec<u32>) -> Result<()> {
        let _ = self;
        let Some(uid) = env.uid() else {
            return Ok(());
        };
        let env = &mut *env.sudo();
        let mut domain = make_domain!([("user", "=", uid), ("read", "=", false)]);
        if !notifications.is_empty() {
            domain = make_domain!([
                ("user", "=", uid),
                ("read", "=", false),
                ("id", "in", notifications)
            ]);
        }
        let found = env.search_ids("notification", &domain)?;
        if found.is_empty() {
            return Ok(());
        }
        let mut values = MapOfFields::default();
        values.insert("read", true);
        env.write("notification", &MultipleIds::from(found), values)?;
        Ok(())
    }
}
