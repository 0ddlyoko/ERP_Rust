use crate::models::{BaseMessageChange, MessageChange};
use base::models::{BaseUsers, Users};
use code_gen::{Model, erp_methods, selection};
use erp::access::Operation;
use erp::environment::Environment;
use erp::internal_types::FinalInternalField;
use erp::model::TrackedChange;
use erp::serde_json::{Value, json};
use erp::types::field::{
    FieldType, IdMode, MultipleIds, Reference, Selection, SingleId, Timestamp, Utc,
};
use erp::types::model::MapOfFields;
use erp_search::{OrderBy, SearchOptions};
use erp_search_code_gen::make_domain;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[selection]
pub enum MessageKind {
    #[default]
    Comment,
    Note,
    Tracking,
}

/// Something said about a record, or noted on it: a message of the record's thread.
///
/// Tied to the record by its model and id rather than by a relation, so that any record has a
/// thread without its model declaring anything.
#[derive(Model)]
#[erp(id = "message", methods)]
#[allow(dead_code)]
pub struct Message<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    model: String,
    #[erp(default = 0)]
    record: i32,
    author: Reference<BaseUsers, SingleId>,
    date: Timestamp,
    kind: MessageKind,
    body: Option<String>,
    #[erp(inverse = "message")]
    changes: Reference<BaseMessageChange, MultipleIds>,
}

/// The models a thread is made of, whose own changes are not noted.
const THREAD_MODELS: [&str; 2] = ["message", "message_change"];

#[erp_methods]
impl Message<MultipleIds> {
    /// The thread of a record, newest first, for whoever may read the record.
    ///
    /// As sudo once the record is checked: a message is readable by whoever reads its record, a
    /// right no access rule on `message` could express.
    #[erp(rpc)]
    pub fn thread(&self, env: &mut Environment, model: String, record: u32) -> Result<Value> {
        let _ = self;
        env.check_access(&model, Operation::Read, &[record], &[])?;
        let env = &mut *env.sudo();
        let options = SearchOptions {
            order: vec![OrderBy::desc("date"), OrderBy::desc("id")],
            ..SearchOptions::new()
        };
        let messages: Message<MultipleIds> = env.search_with(
            &make_domain!([
                ("model", "=", model),
                ("record", "=", i32::try_from(record)?)
            ]),
            &options,
        )?;
        let mut thread = Vec::new();
        for message in messages {
            thread.push(message.describe(env)?);
        }
        Ok(Value::Array(thread))
    }
}

impl Message<SingleId> {
    /// What a client shows of the message: who, when, what kind, and what it says or changed.
    fn describe(&self, env: &mut Environment) -> Result<Value> {
        let author = match self.get_author::<Users<SingleId>>(env)? {
            Some(user) => json!([user.get_id(), user.get_name(env)?]),
            None => Value::Null,
        };
        let changes: MessageChange<MultipleIds> = self.get_changes(env)?;
        let mut lines = Vec::new();
        for change in changes {
            lines.push(json!({
                "field": change.get_field(env)?,
                "label": change.get_label(env)?,
                "old": change.get_old(env)?,
                "new": change.get_new(env)?,
            }));
        }
        Ok(json!({
            "id": self.get_id(),
            "kind": self.get_kind(env)?.key().as_str(),
            "date": self.get_date(env)?.to_rfc3339(),
            "author": author,
            "body": self.get_body(env)?,
            "changes": lines,
        }))
    }
}

/// Note the changes of a record's tracked fields as a message of its thread, by whoever made
/// them. Values are kept as they read now: a label, a record's name.
pub fn note_changes(
    env: &mut Environment,
    model_name: &str,
    record: u32,
    author: Option<u32>,
    changes: &[TrackedChange],
) -> Result<()> {
    if THREAD_MODELS.contains(&model_name) {
        return Ok(());
    }
    let model = env.model_manager.try_get_model(model_name)?;
    let mut lines = Vec::with_capacity(changes.len());
    for change in changes {
        let field = model.try_get_internal_field(&change.field)?;
        if field.private {
            continue;
        }
        lines.push((
            field.name.clone(),
            field.label.clone(),
            shown(env, field, &change.old)?,
            shown(env, field, &change.new)?,
        ));
    }
    if lines.is_empty() {
        return Ok(());
    }
    let env = &mut *env.sudo();
    let mut values = MapOfFields::default();
    values.insert("model", model_name.to_string());
    values.insert("record", i32::try_from(record)?);
    values.insert("date", Utc::now());
    values.insert("kind", MessageKind::Tracking);
    if let Some(author) = author {
        values.insert("author", author);
    }
    let message: Message<SingleId> = env.create_new_record_from_map(values)?;
    for (field, label, old, new) in lines {
        let mut values = MapOfFields::default();
        values.insert("message", message.get_id());
        values.insert("field", field);
        values.insert("label", label);
        if let Some(old) = old {
            values.insert("old", old);
        }
        if let Some(new) = new {
            values.insert("new", new);
        }
        env.create_records("message_change", vec![values])?;
    }
    Ok(())
}

/// A value as a person reads it: a selection's label, the names of the records a relation points
/// to, yes or no.
fn shown(
    env: &mut Environment,
    field: &FinalInternalField,
    value: &Option<FieldType>,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(Some(match value {
        FieldType::String(key) if field.selection.is_some() => env
            .model_manager
            .selections
            .choices(field.selection.map_or("", |family| family.family))
            .iter()
            .find(|choice| &choice.key == key)
            .map_or_else(|| key.clone(), |choice| choice.label.clone()),
        FieldType::Ref(id) => names(env, field, &[*id])?,
        FieldType::Refs(ids) if ids.is_empty() => return Ok(None),
        FieldType::Refs(ids) => names(env, field, ids)?,
        FieldType::Bool(true) => "Yes".to_string(),
        FieldType::Bool(false) => "No".to_string(),
        other => other.to_string(),
    }))
}

/// The names of records a relation points to, in order, separated by commas.
fn names(env: &mut Environment, field: &FinalInternalField, ids: &[u32]) -> Result<String> {
    let target = field
        .inverse
        .as_ref()
        .map(|reference| reference.target_model)
        .ok_or("a relation without a target")?;
    let mut names = env.sudo().names(target, ids)?;
    Ok(ids
        .iter()
        .map(|id| names.remove(id).unwrap_or_else(|| format!("#{id}")))
        .collect::<Vec<_>>()
        .join(", "))
}

/// Delete the thread of records that are gone: nothing else points at it.
pub fn forget_deleted(env: &mut Environment, model_name: &str, ids: &[u32]) -> Result<()> {
    if THREAD_MODELS.contains(&model_name) {
        return Ok(());
    }
    let records: Vec<i32> = ids
        .iter()
        .map(|id| i32::try_from(*id))
        .collect::<std::result::Result<_, _>>()?;
    let env = &mut *env.sudo();
    let messages = env.search_ids(
        "message",
        &make_domain!([
            ("model", "=", model_name.to_string()),
            ("record", "in", records)
        ]),
    )?;
    if messages.is_empty() {
        return Ok(());
    }
    let changes = env.search_ids(
        "message_change",
        &make_domain!([("message", "in", messages.clone())]),
    )?;
    env.delete("message_change", &MultipleIds::from(changes))?;
    env.delete("message", &MultipleIds::from(messages))?;
    Ok(())
}
