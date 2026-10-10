use crate::models::{BaseMessageSubtype, MessageSubtype};
use base::models::{BaseContact, Contact};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::access::Operation;
use erp::environment::Environment;
use erp::search::SearchType;
use erp::serde_json::{Value, json};
use erp::types::field::{FieldType, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;

/// A contact following a record: told of what is said about it and of the changes it follows,
/// as its `subtypes` say. Tied to the record by its model and id, as its thread is.
#[derive(Model)]
#[erp(id = "follower", methods)]
#[allow(dead_code)]
pub struct Follower<Mode: IdMode> {
    pub id: Mode,
    #[erp(index)]
    model: String,
    #[erp(default = 0, index)]
    record: i32,
    #[erp(required, ondelete = "cascade")]
    contact: Reference<BaseContact, SingleId>,
    #[erp(relation = "follower_subtype_rel")]
    subtypes: Reference<BaseMessageSubtype, MultipleIds>,
}

/// The contact a user is, whom they follow records as.
pub(crate) fn contact_of(env: &mut Environment, uid: u32) -> Result<Option<u32>> {
    let env = &mut *env.sudo();
    let rows = env.read("users", &MultipleIds::from(vec![uid]), &["contact"])?;
    Ok(rows
        .first()
        .and_then(|row| row.get_option::<&u32>("contact"))
        .copied())
}

/// The users these contacts are: those told of a message in the application.
pub(crate) fn users_of(env: &mut Environment, contacts: &[u32]) -> Result<Vec<(u32, u32)>> {
    if contacts.is_empty() {
        return Ok(Vec::new());
    }
    let env = &mut *env.sudo();
    let domain = make_domain!([("contact", "in", contacts.to_vec())]);
    let ids = env.search_ids("users", &domain)?;
    let rows = env.read("users", &MultipleIds::from(ids), &["contact"])?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            Some((
                *row.get_option::<&u32>("id")?,
                *row.get_option::<&u32>("contact")?,
            ))
        })
        .collect())
}

/// The contact a record is about, as its model names the field: an order's customer.
pub(crate) fn main_contact(env: &mut Environment, model: &str, record: u32) -> Result<Option<u32>> {
    let Some(field) = env
        .model_manager
        .try_get_model(model)?
        .contact_field
        .clone()
    else {
        return Ok(None);
    };
    let env = &mut *env.sudo();
    let rows = env.read(model, &MultipleIds::from(vec![record]), &[field.as_str()])?;
    Ok(rows
        .first()
        .and_then(|row| row.get_option::<&u32>(&field))
        .copied())
}

/// What may be followed of a record of `model`: its discussions, and the changes of its fields
/// subtypes are declared for. With whether each is followed by default.
pub(crate) fn subtypes_of(env: &mut Environment, model: &str) -> Result<Vec<(u32, String, bool)>> {
    let env = &mut *env.sudo();
    let all: MessageSubtype<MultipleIds> = env.search(&SearchType::Nothing)?;
    let mut found = Vec::new();
    for subtype in all {
        let own = subtype.get_model(env)?.cloned();
        if own.is_none() || own.as_deref() == Some(model) {
            found.push((
                subtype.get_id(),
                subtype.get_name(env)?.clone(),
                *subtype.get_default(env)?,
            ));
        }
    }
    Ok(found)
}

/// The followers of a record.
pub(crate) fn followers_of(
    env: &mut Environment,
    model: &str,
    record: u32,
) -> Result<Follower<MultipleIds>> {
    let env = &mut *env.sudo();
    env.search(&make_domain!([
        ("model", "=", model.to_string()),
        ("record", "=", i32::try_from(record)?)
    ]))
}

/// Make contacts follow a record, those not following it yet, the subtypes followed by default.
pub(crate) fn follow(
    env: &mut Environment,
    model: &str,
    record: u32,
    contacts: &[u32],
) -> Result<()> {
    let followers = followers_of(env, model, record)?;
    let env = &mut *env.sudo();
    let known: Contact<MultipleIds> = followers.get_contact(env)?;
    let defaults: Vec<u32> = subtypes_of(env, model)?
        .into_iter()
        .filter(|(_, _, default)| *default)
        .map(|(id, _, _)| id)
        .collect();
    let mut new = Vec::new();
    for contact in contacts {
        if known.get_ids_ref().contains(contact)
            || new
                .iter()
                .any(|values: &MapOfFields| values.get_option::<&u32>("contact") == Some(contact))
        {
            continue;
        }
        let mut values = MapOfFields::default();
        values.insert("model", model.to_string());
        values.insert("record", i32::try_from(record)?);
        values.insert("contact", *contact);
        values.insert("subtypes", FieldType::Refs(defaults.clone()));
        new.push(values);
    }
    if !new.is_empty() {
        let _: Follower<MultipleIds> = env.create_new_records_from_maps(new)?;
    }
    Ok(())
}

/// The contacts following a record for any of these subtypes.
pub(crate) fn subscribed(
    env: &mut Environment,
    model: &str,
    record: u32,
    subtypes: &[u32],
) -> Result<Vec<u32>> {
    let followers = followers_of(env, model, record)?;
    let env = &mut *env.sudo();
    let mut contacts = Vec::new();
    for follower in followers {
        let followed: MessageSubtype<MultipleIds> = follower.get_subtypes(env)?;
        if followed
            .get_ids_ref()
            .iter()
            .any(|id| subtypes.contains(id))
        {
            contacts.push(follower.get_contact::<Contact<SingleId>>(env)?.get_id());
        }
    }
    Ok(contacts)
}

#[erp_methods]
impl Follower<MultipleIds> {
    /// Who follows a record and what of it, what may be followed, the contact it is about, and
    /// whether the user follows it — for whoever may read the record.
    #[erp(rpc)]
    pub fn of(&self, env: &mut Environment, model: String, record: u32) -> Result<Value> {
        let _ = self;
        env.check_access(&model, Operation::Read, &[record], &[])?;
        let me = match env.uid() {
            Some(uid) => contact_of(env, uid)?,
            None => None,
        };
        let main = main_contact(env, &model, record)?;
        let subtypes = subtypes_of(env, &model)?;
        let followers = followers_of(env, &model, record)?;
        let env = &mut *env.sudo();
        let mut contacts = Vec::new();
        let mut rows = Vec::new();
        for follower in followers {
            let contact = follower.get_contact::<Contact<SingleId>>(env)?.get_id();
            let followed: MessageSubtype<MultipleIds> = follower.get_subtypes(env)?;
            contacts.push(contact);
            rows.push((follower.get_id(), contact, followed.get_ids_ref().to_vec()));
        }
        contacts.extend(main);
        let names = env.names("contact", &contacts)?;
        let name = |id: u32| {
            json!([
                id,
                names.get(&id).cloned().unwrap_or_else(|| format!("#{id}"))
            ])
        };
        Ok(json!({
            "following": me.is_some_and(|me| rows.iter().any(|(_, contact, _)| *contact == me)),
            "me": me,
            "contact": main.map(name),
            "followers": rows
                .iter()
                .map(|(id, contact, followed)| json!({"id": id, "contact": name(*contact), "subtypes": followed}))
                .collect::<Vec<_>>(),
            "subtypes": subtypes
                .iter()
                .map(|(id, label, _)| json!({"id": id, "name": label}))
                .collect::<Vec<_>>(),
        }))
    }

    /// The user follows a record they may read.
    #[erp(rpc)]
    pub fn follow(&self, env: &mut Environment, model: String, record: u32) -> Result<()> {
        let _ = self;
        env.check_access(&model, Operation::Read, &[record], &[])?;
        let me = env
            .uid()
            .map(|uid| contact_of(env, uid))
            .transpose()?
            .flatten();
        let me = me.ok_or("Only a user with a contact follows a record")?;
        follow(env, &model, record, &[me])
    }

    /// The user stops following a record.
    #[erp(rpc)]
    pub fn unfollow(&self, env: &mut Environment, model: String, record: u32) -> Result<()> {
        let _ = self;
        env.check_access(&model, Operation::Read, &[record], &[])?;
        let Some(me) = env
            .uid()
            .map(|uid| contact_of(env, uid))
            .transpose()?
            .flatten()
        else {
            return Ok(());
        };
        let followers = followers_of(env, &model, record)?;
        let env = &mut *env.sudo();
        for follower in followers {
            if follower.get_contact::<Contact<SingleId>>(env)?.get_id() == me {
                follower.delete(env)?;
            }
        }
        Ok(())
    }

    /// Make others follow a record, for whoever may change it.
    #[erp(rpc)]
    pub fn add(
        &self,
        env: &mut Environment,
        model: String,
        record: u32,
        contacts: Vec<u32>,
    ) -> Result<()> {
        let _ = self;
        env.check_access(&model, Operation::Write, &[record], &[])?;
        follow(env, &model, record, &contacts)
    }

    /// Stop a follower following, for the follower themselves or whoever may change the record.
    #[erp(rpc)]
    pub fn forget(&self, env: &mut Environment, follower: u32) -> Result<()> {
        let _ = self;
        let follower = Self::own_or_changeable(env, follower)?;
        follower.delete(&mut *env.sudo())?;
        Ok(())
    }

    /// What a follower follows of the record, for the follower themselves or whoever may change it.
    #[erp(rpc)]
    pub fn subscribe(
        &self,
        env: &mut Environment,
        follower: u32,
        subtypes: Vec<u32>,
    ) -> Result<()> {
        let _ = self;
        let follower = Self::own_or_changeable(env, follower)?;
        let mut values = MapOfFields::default();
        values.insert("subtypes", FieldType::Refs(subtypes));
        env.sudo().write(
            "follower",
            &MultipleIds::from(vec![follower.get_id()]),
            values,
        )?;
        Ok(())
    }
}

impl Follower<MultipleIds> {
    /// A follower the user is, or of a record the user may change.
    fn own_or_changeable(env: &mut Environment, follower: u32) -> Result<Follower<SingleId>> {
        let found: Follower<SingleId> = env.get_record(SingleId::from(follower));
        let (model, record, contact) = {
            let env = &mut *env.sudo();
            env.existing("follower", vec![follower])?;
            (
                found.get_model(env)?.clone(),
                u32::try_from(*found.get_record(env)?)?,
                found.get_contact::<Contact<SingleId>>(env)?.get_id(),
            )
        };
        let me = env
            .uid()
            .map(|uid| contact_of(env, uid))
            .transpose()?
            .flatten();
        if me != Some(contact) {
            env.check_access(&model, Operation::Write, &[record], &[])?;
        }
        Ok(found)
    }
}
