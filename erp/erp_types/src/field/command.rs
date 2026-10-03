use crate::field::{FieldKinds, deserialize::MapOfFieldsVisitor};
use crate::model::MapOfFields;
use serde::de::{DeserializeSeed, Deserializer, Error, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use std::fmt;

/// Something to do to the records a one2many or a many2many holds, carried out on what it holds.
///
/// On the wire, an object holding any of them — `{"unlink": [3], "create": [{...}]}` — applied
/// in the order of this enum's variants: clear, unlink, delete, update, create, link. A list of
/// such objects is applied one object after the other.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Take every record out, as `Unlink` would.
    Clear,
    /// Take these records out: deleted if the one2many owns them, else let go.
    Unlink(Vec<u32>),
    /// Take these records out and delete them.
    Delete(Vec<u32>),
    /// Change these records, then hold them.
    Update(Vec<(u32, MapOfFields)>),
    /// Create these records and hold them — for a one2many, pointing back already.
    Create(Vec<MapOfFields>),
    /// Hold these records.
    Link(Vec<u32>),
}

/// The keys of a commands object, in the order its commands are carried out.
const KEYS: [&str; 6] = ["clear", "unlink", "delete", "update", "create", "link"];

impl serde::Serialize for Command {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Command::Clear => map.serialize_entry("clear", &true)?,
            Command::Unlink(ids) => map.serialize_entry("unlink", ids)?,
            Command::Delete(ids) => map.serialize_entry("delete", ids)?,
            Command::Update(records) => {
                let records: Vec<UpdatedRecord> = records
                    .iter()
                    .map(|(id, values)| UpdatedRecord(*id, values))
                    .collect();
                map.serialize_entry("update", &records)?;
            }
            Command::Create(records) => map.serialize_entry("create", records)?,
            Command::Link(ids) => map.serialize_entry("link", ids)?,
        }
        map.end()
    }
}

/// A record to change, written as its values with its `id` among them.
struct UpdatedRecord<'a>(u32, &'a MapOfFields);

impl serde::Serialize for UpdatedRecord<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.1.fields.len() + 1))?;
        map.serialize_entry("id", &self.0)?;
        for (name, value) in &self.1.fields {
            map.serialize_entry(name, value)?;
        }
        map.end()
    }
}

/// The commands of one object, read against the fields of the model the relation points to.
pub struct CommandsSeed<'a>(pub &'a dyn FieldKinds);

impl<'de> DeserializeSeed<'de> for CommandsSeed<'_> {
    type Value = Vec<Command>;

    fn deserialize<D>(self, deserializer: D) -> Result<Vec<Command>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(CommandsVisitor(self.0))
    }
}

struct CommandsVisitor<'a>(&'a dyn FieldKinds);

impl<'de> CommandsVisitor<'_> {
    /// Read the commands of an object, its first key already taken off it.
    pub(crate) fn read_from<M>(self, first: String, mut map: M) -> Result<Vec<Command>, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut found: Vec<(usize, Command)> = Vec::new();
        let mut next = Some(first);
        while let Some(key) = next.take() {
            let command = match key.as_str() {
                "clear" => map.next_value::<bool>()?.then_some(Command::Clear),
                "unlink" => Some(Command::Unlink(map.next_value()?)),
                "delete" => Some(Command::Delete(map.next_value()?)),
                "link" => Some(Command::Link(map.next_value()?)),
                "create" => Some(Command::Create(
                    map.next_value_seed(Records {
                        kinds: self.0,
                        with_id: false,
                    })?
                    .into_iter()
                    .map(|(_, values)| values)
                    .collect(),
                )),
                "update" => {
                    let records = map.next_value_seed(Records {
                        kinds: self.0,
                        with_id: true,
                    })?;
                    let mut updated = Vec::with_capacity(records.len());
                    for (id, values) in records {
                        let id = id.ok_or_else(|| {
                            M::Error::custom("a record to update names itself by its \"id\"")
                        })?;
                        updated.push((id, values));
                    }
                    Some(Command::Update(updated))
                }
                other => {
                    return Err(M::Error::custom(format!(
                        "\"{other}\" is not a command: {}",
                        KEYS.join(", ")
                    )));
                }
            };
            if let Some(command) = command {
                let order = KEYS
                    .iter()
                    .position(|known| *known == key)
                    .unwrap_or(KEYS.len());
                found.push((order, command));
            }
            next = map.next_key::<String>()?;
        }
        found.sort_by_key(|(order, _)| *order);
        Ok(found.into_iter().map(|(_, command)| command).collect())
    }
}

impl<'de> Visitor<'de> for CommandsVisitor<'_> {
    type Value = Vec<Command>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "an object of commands: {}", KEYS.join(", "))
    }

    fn visit_map<M>(self, mut map: M) -> Result<Vec<Command>, M::Error>
    where
        M: MapAccess<'de>,
    {
        match map.next_key::<String>()? {
            Some(first) => self.read_from(first, map),
            None => Ok(Vec::new()),
        }
    }
}

/// What a one2many or a many2many is written with: ids alone, which it then holds, or commands,
/// carried out on what it holds — one object of them, or a list of objects in order.
pub enum RelationValue {
    Ids(Vec<u32>),
    Commands(Vec<Command>),
}

pub(crate) struct RelationValueSeed<'a>(pub &'a dyn FieldKinds);

impl<'de> DeserializeSeed<'de> for RelationValueSeed<'_> {
    type Value = RelationValue;

    fn deserialize<D>(self, deserializer: D) -> Result<RelationValue, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(RelationValueVisitor(self.0))
    }
}

struct RelationValueVisitor<'a>(&'a dyn FieldKinds);

impl<'de> Visitor<'de> for RelationValueVisitor<'_> {
    type Value = RelationValue;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "a list of ids, or commands — an object of {}, or a list of such objects",
            KEYS.join(", ")
        )
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<RelationValue, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut ids = Vec::new();
        let mut commands = Vec::new();
        while let Some(element) = seq.next_element_seed(ElementSeed(self.0))? {
            match element {
                Element::Id(id) => ids.push(id),
                Element::Commands(more) => commands.extend(more),
            }
            if !ids.is_empty() && !commands.is_empty() {
                return Err(S::Error::custom(
                    "a list holds ids, or objects of commands, not both",
                ));
            }
        }
        Ok(if commands.is_empty() {
            RelationValue::Ids(ids)
        } else {
            RelationValue::Commands(commands)
        })
    }

    fn visit_map<M>(self, map: M) -> Result<RelationValue, M::Error>
    where
        M: MapAccess<'de>,
    {
        CommandsVisitor(self.0)
            .visit_map(map)
            .map(RelationValue::Commands)
    }
}

enum Element {
    Id(u32),
    Commands(Vec<Command>),
}

struct ElementSeed<'a>(&'a dyn FieldKinds);

impl<'de> DeserializeSeed<'de> for ElementSeed<'_> {
    type Value = Element;

    fn deserialize<D>(self, deserializer: D) -> Result<Element, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ElementVisitor(self.0))
    }
}

struct ElementVisitor<'a>(&'a dyn FieldKinds);

impl<'de> Visitor<'de> for ElementVisitor<'_> {
    type Value = Element;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("the id of a record, or an object of commands")
    }

    fn visit_u64<E: Error>(self, id: u64) -> Result<Element, E> {
        u32::try_from(id)
            .map(Element::Id)
            .map_err(|_| E::custom(format!("{id} is not the id of a record")))
    }

    fn visit_i64<E: Error>(self, id: i64) -> Result<Element, E> {
        u32::try_from(id)
            .map(Element::Id)
            .map_err(|_| E::custom(format!("{id} is not the id of a record")))
    }

    fn visit_map<M>(self, map: M) -> Result<Element, M::Error>
    where
        M: MapAccess<'de>,
    {
        CommandsVisitor(self.0)
            .visit_map(map)
            .map(Element::Commands)
    }
}

/// A list of records' values, each maybe with its `id`.
struct Records<'a> {
    kinds: &'a dyn FieldKinds,
    with_id: bool,
}

impl<'de> DeserializeSeed<'de> for Records<'_> {
    type Value = Vec<(Option<u32>, MapOfFields)>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for Records<'_> {
    type Value = Vec<(Option<u32>, MapOfFields)>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a list of records' values")
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut records = Vec::new();
        while let Some(record) = seq.next_element_seed(Record {
            kinds: self.kinds,
            with_id: self.with_id,
        })? {
            records.push(record);
        }
        Ok(records)
    }
}

struct Record<'a> {
    kinds: &'a dyn FieldKinds,
    with_id: bool,
}

impl<'de> DeserializeSeed<'de> for Record<'_> {
    type Value = (Option<u32>, MapOfFields);

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Record<'_> {
    type Value = (Option<u32>, MapOfFields);

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a record's values")
    }

    fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let visitor = if self.with_id {
            MapOfFieldsVisitor::with_id(self.kinds)
        } else {
            MapOfFieldsVisitor::without_id(self.kinds)
        };
        visitor.read(map)
    }
}
