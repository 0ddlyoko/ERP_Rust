use crate::models::follower::{
    contact_of, follow, main_contact, subscribed, subtypes_of, users_of,
};
use crate::models::{
    BaseMessageChange, BaseMessageSubtype, MessageChange, MessageSubtype, NotificationReason,
};
use base::models::{BaseContact, BaseUsers, Contact, Users};
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::access::Operation;
use erp::environment::Environment;
use erp::internal_types::FinalInternalField;
use erp::model::{ModelVerbs, TrackedChange};
use erp::serde_json::{Value, json};
use erp::types::field::{
    FieldType, IdMode, MultipleIds, Reference, Selection, SingleId, Timestamp, Utc,
};
use erp::types::model::MapOfFields;
use erp_search::{OrderBy, SearchOptions, SearchType};
use erp_search_code_gen::make_domain;

#[selection]
pub enum MessageKind {
    #[default]
    Comment,
    Note,
    Tracking,
    Creation,
}

/// Something said about a record, or noted on it: a message of the record's thread.
///
/// Tied to the record by its model and id rather than by a relation, so that any record has a
/// thread without its model declaring anything. A comment is said to the record's followers and
/// to the contact it is about — its `recipients`, kept for the mail to come — a note only to the
/// people it mentions; a change of fields belongs to the `subtype` its followers follow.
#[derive(Model)]
#[erp(id = "message", methods)]
#[allow(dead_code)]
pub struct Message<Mode: IdMode> {
    pub id: Mode,
    #[erp(index)]
    model: String,
    #[erp(default = 0, index)]
    record: i32,
    author: Reference<BaseUsers, SingleId>,
    #[erp(index)]
    date: Timestamp,
    kind: MessageKind,
    body: Option<String>,
    #[erp(inverse = "message")]
    changes: Reference<BaseMessageChange, MultipleIds>,
    #[erp(ondelete = "set_null")]
    subtype: Reference<BaseMessageSubtype, SingleId>,
    #[erp(relation = "message_recipient_rel")]
    recipients: Reference<BaseContact, MultipleIds>,
    #[erp(relation = "message_mention_rel")]
    mentions: Reference<BaseContact, MultipleIds>,
}

/// The models a thread is made of, whose own changes are not noted.
const THREAD_MODELS: [&str; 6] = [
    "message",
    "message_change",
    "message_subtype",
    "follower",
    "notification",
    "mail",
];

/// The subtype of what is said about any record.
const DISCUSSION: &str = "mail.subtype_discussion";

#[erp_methods]
impl Message<MultipleIds> {
    /// The thread of a record, newest first, for whoever may read the record; given a `field`,
    /// only the messages noting a change of it; given `kinds`, only messages of those —
    /// `comment`, `note`, `tracking`, `creation`. A page of it: `limit` messages past the
    /// `offset` first ones, all of them without a limit.
    ///
    /// As sudo once the record is checked: a message is readable by whoever reads its record, a
    /// right no access rule on `message` could express.
    #[erp(rpc)]
    pub fn thread(
        &self,
        env: &mut Environment,
        model: String,
        record: u32,
        field: Option<String>,
        kinds: Option<Vec<String>>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Value> {
        let _ = self;
        env.check_access(&model, Operation::Read, &[record], &[])?;
        let env = &mut *env.sudo();
        let mut options = SearchOptions {
            order: vec![OrderBy::desc("date"), OrderBy::desc("id")],
            ..SearchOptions::new()
        }
        .with_offset(offset.unwrap_or(0));
        if let Some(limit) = limit {
            options = options.with_limit(limit);
        }
        let mut domain = make_domain!([
            ("model", "=", model),
            ("record", "=", i32::try_from(record)?)
        ]);
        if let Some(field) = field {
            domain = SearchType::And(
                Box::new(domain),
                Box::new(make_domain!([("changes.field", "=", field)])),
            );
        }
        if let Some(kinds) = kinds {
            domain = SearchType::And(
                Box::new(domain),
                Box::new(make_domain!([("kind", "in", kinds)])),
            );
        }
        let messages: Message<MultipleIds> = env.search_with(&domain, &options)?;
        let mut thread = Vec::new();
        for message in messages {
            thread.push(message.describe(env)?);
        }
        Ok(Value::Array(thread))
    }

    /// Say something about a record the user may read: a comment, to its followers of its
    /// discussions and the contact it is about, or an `internal` note. The contacts it
    /// `mentions` are told of it either way; the users among those it is said to find it in their
    /// inbox. Whoever says it follows the record from then on.
    #[erp(rpc)]
    pub fn post(
        &self,
        env: &mut Environment,
        model: String,
        record: u32,
        body: String,
        internal: bool,
        mentions: Vec<u32>,
    ) -> Result<Value> {
        let _ = self;
        env.check_access(&model, Operation::Read, &[record], &[])?;
        if body.trim().is_empty() {
            return Err("A message says something".into());
        }
        let author = env.uid();
        let me = author
            .map(|uid| contact_of(env, uid))
            .transpose()?
            .flatten();
        let discussion = if internal {
            None
        } else {
            erp::data::resolve(&mut env.sudo(), DISCUSSION)?
        };
        let mut recipients = Vec::new();
        if !internal {
            if let Some(discussion) = discussion {
                recipients.extend(subscribed(env, &model, record, &[discussion])?);
            }
            recipients.extend(main_contact(env, &model, record)?);
            recipients.extend(mentions.iter().copied());
        }
        recipients.sort_unstable();
        recipients.dedup();
        recipients.retain(|contact| Some(*contact) != me);

        let env = &mut *env.sudo();
        let mut values = MapOfFields::default();
        values.insert("model", model.clone());
        values.insert("record", i32::try_from(record)?);
        values.insert("date", Utc::now());
        values.insert(
            "kind",
            if internal {
                MessageKind::Note
            } else {
                MessageKind::Comment
            },
        );
        values.insert("body", body);
        values.insert("recipients", FieldType::Refs(recipients.clone()));
        values.insert("mentions", FieldType::Refs(mentions.clone()));
        if let Some(author) = author {
            values.insert("author", author);
        }
        if let Some(discussion) = discussion {
            values.insert("subtype", discussion);
        }
        let posted: Message<MultipleIds> = env.create_new_records_from_maps(vec![values])?;
        let message = posted.get_ids_ref()[0];

        let mut told = Vec::new();
        for (user, _) in users_of(env, &mentions)? {
            told.push((user, NotificationReason::Mention));
        }
        for (user, _) in users_of(env, &recipients)? {
            told.push((user, NotificationReason::Follower));
        }
        notify(env, message, author, told)?;
        queue_mails(env, message, &model, record, &recipients)?;
        if let Some(me) = me {
            follow(env, &model, record, &[me])?;
        }
        let posted: Message<SingleId> = env.get_record(SingleId::from(message));
        posted.describe(env)
    }
}

/// Queue a mail of a message to each recipient who is no user of the application — users find
/// it in their inbox — at their address as it is now; one without an address is a mail failed
/// already, for the thread to say so.
fn queue_mails(
    env: &mut Environment,
    message: u32,
    model: &str,
    record: u32,
    recipients: &[u32],
) -> Result<()> {
    let users: Vec<u32> = users_of(env, recipients)?
        .into_iter()
        .map(|(_, contact)| contact)
        .collect();
    let outside: Vec<u32> = recipients
        .iter()
        .copied()
        .filter(|contact| !users.contains(contact))
        .collect();
    if outside.is_empty() {
        return Ok(());
    }
    let env = &mut *env.sudo();
    let subject = env.names(model, &[record])?.remove(&record);
    let body: Option<String> = env
        .read("message", &MultipleIds::from(vec![message]), &["body"])?
        .first()
        .and_then(|row| row.get_option::<&String>("body").cloned());
    let rows = env.read("contact", &MultipleIds::from(outside), &["email"])?;
    let mut mails = Vec::new();
    for row in rows {
        let Some(contact) = row.get_option::<&u32>("id").copied() else {
            continue;
        };
        let email = row
            .get_option::<&String>("email")
            .cloned()
            .filter(|email| !email.trim().is_empty());
        let mut values = MapOfFields::default();
        values.insert("message", message);
        values.insert("recipient", contact);
        if let Some(subject) = &subject {
            values.insert("subject", subject.clone());
        }
        if let Some(body) = &body {
            values.insert("body", body.clone());
        }
        match email {
            Some(email) => values.insert("email", email),
            None => {
                values.insert("state", crate::models::MailState::Failed);
                values.insert("error", "No address to send it to");
            }
        }
        mails.push(values);
    }
    let _: crate::models::Mail<MultipleIds> = env.create_new_records_from_maps(mails)?;
    Ok(())
}

/// Tell users of a message in their inbox — each once, a mention before a follow — but its author.
pub(crate) fn notify(
    env: &mut Environment,
    message: u32,
    author: Option<u32>,
    told: Vec<(u32, NotificationReason)>,
) -> Result<()> {
    let mut seen = Vec::new();
    let mut notifications = Vec::new();
    for (user, reason) in told {
        if Some(user) == author || seen.contains(&user) {
            continue;
        }
        seen.push(user);
        let mut values = MapOfFields::default();
        values.insert("message", message);
        values.insert("user", user);
        values.insert("reason", reason);
        notifications.push(values);
    }
    if !notifications.is_empty() {
        let _: crate::models::Notification<MultipleIds> =
            env.sudo().create_new_records_from_maps(notifications)?;
    }
    Ok(())
}

#[erp_methods]
impl Message<SingleId> {
    /// What a client shows of the message: who, when, what kind, what it says or changed, whom
    /// it was said to and whom it mentions.
    pub(crate) fn describe(&self, env: &mut Environment) -> Result<Value> {
        let user = self.get_author::<Users<SingleId>>(env)?;
        let author = if user.is_empty() {
            Value::Null
        } else {
            json!([user.get_id(), user.get_name(env)?])
        };
        let changes: MessageChange<MultipleIds> = self.get_changes(env)?;
        let mut lines = Vec::new();
        for change in changes {
            lines.push(json!({
                "field": change.get_field(env)?,
                "label": change.get_label(env)?,
                "old": change.get_old(env)?,
                "new": change.get_new(env)?,
                "old_value": change.get_old_value(env)?,
                "new_value": change.get_new_value(env)?,
            }));
        }
        let subtype: MessageSubtype<SingleId> = self.get_subtype(env)?;
        let subtype = if subtype.is_empty() {
            Value::Null
        } else {
            json!(subtype.get_name(env)?)
        };
        let mails = mails_of(env, self.get_id())?;
        let recipients: Contact<MultipleIds> = self.get_recipients(env)?;
        let mentions: Contact<MultipleIds> = self.get_mentions(env)?;
        let named = |env: &mut Environment, ids: &[u32]| -> Result<Value> {
            let names = env.names("contact", ids)?;
            Ok(json!(
                ids.iter()
                    .map(|id| json!([id, names.get(id)]))
                    .collect::<Vec<_>>()
            ))
        };
        Ok(json!({
            "id": self.get_id(),
            "kind": self.get_kind(env)?.key().as_str(),
            "date": self.get_date(env)?.to_rfc3339(),
            "author": author,
            "body": self.get_body(env)?,
            "changes": lines,
            "subtype": subtype,
            "recipients": named(env, recipients.get_ids_ref())?,
            "mentions": named(env, mentions.get_ids_ref())?,
            "mails": mails,
        }))
    }
}

/// The mails a message was sent as: to whom, at which address, and where each stands.
fn mails_of(env: &mut Environment, message: u32) -> Result<Value> {
    let env = &mut *env.sudo();
    let ids = env.search_ids("mail", &make_domain!([("message", "=", message)]))?;
    if ids.is_empty() {
        return Ok(json!([]));
    }
    let rows = env.read(
        "mail",
        &MultipleIds::from(ids),
        &["recipient", "email", "state", "error"],
    )?;
    let contacts: Vec<u32> = rows
        .iter()
        .filter_map(|row| row.get_option::<&u32>("recipient").copied())
        .collect();
    let names = env.names("contact", &contacts)?;
    Ok(json!(
        rows.iter()
            .map(|row| {
                let recipient = row.get_option::<&u32>("recipient").copied();
                json!({
                    "recipient": recipient.and_then(|id| names.get(&id).cloned()),
                    "email": row.get_option::<&String>("email"),
                    "state": row.get_option::<&String>("state"),
                    "error": row.get_option::<&String>("error"),
                })
            })
            .collect::<Vec<_>>()
    ))
}

/// Note the changes of a record's tracked fields as a message of its thread, by whoever made
/// them. Values are kept as they read now — a label, a record's name — and as they are stored.
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
        if !field.private {
            lines.push(line(env, field, &change.old, &change.new)?);
        }
    }
    if lines.is_empty() {
        return Ok(());
    }
    let fields: Vec<String> = lines.iter().map(|line| line.field.clone()).collect();
    let posted = post(
        env,
        model_name,
        author,
        MessageKind::Tracking,
        vec![(record, lines)],
    )?;
    tell_followers(env, model_name, record, posted, author, &fields)
}

/// Tell the followers of a record of a change of fields they follow: those of a subtype naming
/// one of the fields, the message noted as of the first.
fn tell_followers(
    env: &mut Environment,
    model_name: &str,
    record: u32,
    posted: Message<MultipleIds>,
    author: Option<u32>,
    fields: &[String],
) -> Result<()> {
    let Some(&message) = posted.get_ids_ref().first() else {
        return Ok(());
    };
    let env = &mut *env.sudo();
    let mut matching = Vec::new();
    for (id, _, _) in subtypes_of(env, model_name)? {
        let subtype: MessageSubtype<SingleId> = env.get_record(SingleId::from(id));
        let field = subtype.get_field(env)?.cloned();
        if subtype.get_model(env)?.is_some() && field.is_some_and(|field| fields.contains(&field)) {
            matching.push(id);
        }
    }
    let Some(&first) = matching.first() else {
        return Ok(());
    };
    let mut values = MapOfFields::default();
    values.insert("subtype", first);
    env.write("message", &MultipleIds::from(vec![message]), values)?;
    let contacts = subscribed(env, model_name, record, &matching)?;
    let told = users_of(env, &contacts)?
        .into_iter()
        .map(|(user, _)| (user, NotificationReason::Follower))
        .collect();
    notify(env, message, author, told)
}

/// Note the creation of records whose model tracks fields, with what those fields start as: the
/// start of what their thread will note the changes of.
pub fn note_creation(env: &mut Environment, model_name: &str, ids: &[u32]) -> Result<()> {
    if THREAD_MODELS.contains(&model_name) {
        return Ok(());
    }
    let model = env.model_manager.try_get_model(model_name)?;
    let tracked: Vec<&FinalInternalField> = model
        .fields
        .values()
        .filter(|field| field.tracking && !field.private)
        .collect();
    if tracked.is_empty() {
        return Ok(());
    }
    let names: Vec<&str> = tracked.iter().map(|field| field.name.as_str()).collect();
    let author = env.uid();
    let rows = env
        .sudo()
        .read(model_name, &MultipleIds::from(ids.to_vec()), &names)?;
    let mut created = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(id) = row.get_option::<&u32>("id").copied() else {
            continue;
        };
        let mut lines = Vec::new();
        for field in &tracked {
            let value = row.fields.get(&field.name).cloned().flatten();
            if stored(&value).is_some() {
                lines.push(line(env, field, &None, &value)?);
            }
        }
        created.push((id, lines));
    }
    let records: Vec<u32> = created.iter().map(|(id, _)| *id).collect();
    post(env, model_name, author, MessageKind::Creation, created)?;
    let Some(me) = author
        .map(|uid| contact_of(env, uid))
        .transpose()?
        .flatten()
    else {
        return Ok(());
    };
    for record in records {
        follow(env, model_name, record, &[me])?;
    }
    Ok(())
}

/// One change as a message keeps it: the field, its label, and its values before and after, as
/// shown and as stored.
struct Line {
    field: String,
    label: String,
    old: (Option<String>, Option<String>),
    new: (Option<String>, Option<String>),
}

fn line(
    env: &mut Environment,
    field: &FinalInternalField,
    old: &Option<FieldType>,
    new: &Option<FieldType>,
) -> Result<Line> {
    Ok(Line {
        field: field.name.clone(),
        label: field.label.clone(),
        old: (shown(env, field, old)?, stored(old)),
        new: (shown(env, field, new)?, stored(new)),
    })
}

/// Add a message to each record's thread, with the changes it notes, as its author: the message
/// is theirs, whoever this unit of work runs as by now. Without rights checked, since whoever may
/// change a record may leave the trace of it. All the messages are created at once, then all
/// their lines.
fn post(
    env: &mut Environment,
    model_name: &str,
    author: Option<u32>,
    kind: MessageKind,
    messages: Vec<(u32, Vec<Line>)>,
) -> Result<Message<MultipleIds>> {
    if messages.is_empty() {
        return Ok(env.get_record(MultipleIds::default()));
    }
    let mut as_author = match author {
        Some(author) => env.as_user(author),
        None => env.sudo(),
    };
    let env = &mut *as_author.sudo();
    let date = Utc::now();
    let mut heads = Vec::with_capacity(messages.len());
    for (record, _) in &messages {
        let mut values = MapOfFields::default();
        values.insert("model", model_name.to_string());
        values.insert("record", i32::try_from(*record)?);
        values.insert("date", date);
        values.insert("kind", kind);
        if let Some(author) = author {
            values.insert("author", author);
        }
        heads.push(values);
    }
    let posted: Message<MultipleIds> = env.create_new_records_from_maps(heads)?;
    let mut changes = Vec::new();
    for (message, (_, lines)) in posted.get_ids_ref().iter().zip(messages) {
        for line in lines {
            let mut values = MapOfFields::default();
            values.insert("message", *message);
            values.insert("field", line.field);
            values.insert("label", line.label);
            for (name, value) in [
                ("old", line.old.0),
                ("old_value", line.old.1),
                ("new", line.new.0),
                ("new_value", line.new.1),
            ] {
                if let Some(value) = value {
                    values.insert(name, value);
                }
            }
            changes.push(values);
        }
    }
    if !changes.is_empty() {
        let _: MessageChange<MultipleIds> = env.create_new_records_from_maps(changes)?;
    }
    Ok(posted)
}

/// A value as it is stored, as text: a selection's key, a record's id, ids separated by commas.
fn stored(value: &Option<FieldType>) -> Option<String> {
    Some(match value.as_ref()? {
        FieldType::Refs(ids) if ids.is_empty() => return None,
        FieldType::Refs(ids) => ids.iter().map(u32::to_string).collect::<Vec<_>>().join(","),
        FieldType::Password(_) => return None,
        other => other.to_string(),
    })
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
    let followers = crate::models::Follower::<MultipleIds>::search(
        &make_domain!([
            ("model", "=", model_name.to_string()),
            ("record", "in", records.clone())
        ]),
        env,
    )?;
    followers.delete(env)?;
    let messages = Message::<MultipleIds>::search(
        &make_domain!([
            ("model", "=", model_name.to_string()),
            ("record", "in", records)
        ]),
        env,
    )?;
    if messages.id.is_empty() {
        return Ok(());
    }
    let changes: MessageChange<MultipleIds> = messages.get_changes(env)?;
    changes.delete(env)?;
    messages.delete(env)?;
    Ok(())
}
