//! Reading field values off the wire.
//!
//! A value arrives bare — `7`, `"0ddlyoko"`, `[1, 2]` — because that is how it goes out. Nothing
//! in it says whether `7` is an integer or the id of a related record, so deserialization is
//! driven by the kind the model declares, never by the shape of the incoming value.
//!
//! Expressed as [`serde::de::DeserializeSeed`] rather than a function over a parsed JSON tree:
//! the seed carries the kind, works with any format, and keeps this crate free of a dependency on
//! one.

use crate::field::{FieldKind, FieldType, Password, RelationValue, RelationValueSeed};
use crate::model::MapOfFields;
use serde::Deserialize;
use serde::de::{DeserializeSeed, Deserializer, Error, MapAccess, Visitor};
use std::fmt;

impl<'de> DeserializeSeed<'de> for FieldKind {
    type Value = FieldType;

    fn deserialize<D>(self, deserializer: D) -> Result<FieldType, D::Error>
    where
        D: Deserializer<'de>,
    {
        match self {
            FieldKind::String => String::deserialize(deserializer).map(FieldType::String),
            FieldKind::Integer => i32::deserialize(deserializer).map(FieldType::Integer),
            FieldKind::Bool => bool::deserialize(deserializer).map(FieldType::Bool),
            FieldKind::Ref => u32::deserialize(deserializer).map(FieldType::Ref),
            FieldKind::Refs => Vec::<u32>::deserialize(deserializer).map(FieldType::Refs),
            FieldKind::Decimal => deserializer.deserialize_any(DecimalVisitor),
            // Both go out as strings, and their parsing already exists for the data files.
            FieldKind::Date | FieldKind::DateTime => {
                let raw = String::deserialize(deserializer)?;
                self.parse(&raw).map_err(Error::custom)
            }
            // What arrives is the clear password, and what is kept is its hash: hashing happens
            // here, at the edge, so no route into the ORM can carry a password any further.
            FieldKind::Password => {
                let clear = String::deserialize(deserializer)?;
                Password::new(&clear)
                    .map(FieldType::Password)
                    .map_err(Error::custom)
            }
        }
    }
}

/// Accepts a decimal written as a string or as a whole number, and refuses a fractional one.
///
/// A JSON fraction arrives as a float, which is the one thing `Decimal` exists to avoid: taking
/// it would silently round the value the caller sent. Refusing says so instead.
struct DecimalVisitor;

impl Visitor<'_> for DecimalVisitor {
    type Value = FieldType;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a decimal, as a string such as \"12.34\" or as a whole number")
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<FieldType, E> {
        FieldKind::Decimal.parse(value).map_err(Error::custom)
    }

    fn visit_i64<E: Error>(self, value: i64) -> Result<FieldType, E> {
        Ok(FieldType::Decimal(value.into()))
    }

    fn visit_u64<E: Error>(self, value: u64) -> Result<FieldType, E> {
        Ok(FieldType::Decimal(value.into()))
    }

    fn visit_f64<E: Error>(self, value: f64) -> Result<FieldType, E> {
        Err(Error::custom(format!(
            "{value} is a decimal written as a fraction, which cannot be read exactly. Send it as \
             a string instead."
        )))
    }
}

/// A value of this kind, or nothing.
///
/// `null` means the field is explicitly empty, which is not the same as leaving it out: one
/// clears the value, the other keeps it.
pub struct OptionalValue(pub FieldKind);

impl<'de> DeserializeSeed<'de> for OptionalValue {
    type Value = Option<FieldType>;

    fn deserialize<D>(self, deserializer: D) -> Result<Option<FieldType>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_option(OptionalValueVisitor(self.0))
    }
}

struct OptionalValueVisitor(FieldKind);

impl<'de> Visitor<'de> for OptionalValueVisitor {
    type Value = Option<FieldType>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a value of kind {}, or null", self.0)
    }

    fn visit_none<E: Error>(self) -> Result<Option<FieldType>, E> {
        Ok(None)
    }

    fn visit_unit<E: Error>(self) -> Result<Option<FieldType>, E> {
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Option<FieldType>, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.0.deserialize(deserializer).map(Some)
    }
}

/// Where the kind of each field comes from.
///
/// A trait rather than a map, so the registry can answer straight from the model it already
/// holds instead of a copy built per request.
pub trait FieldKinds {
    fn kind_of(&self, field_name: &str) -> Option<FieldKind>;

    /// The fields of the model a one2many or a many2many points to, for the records it creates
    /// or changes; none when only ids are taken.
    fn target(&self, field_name: &str) -> Option<Box<dyn FieldKinds + '_>> {
        let _ = field_name;
        None
    }
}

impl FieldKinds for std::collections::HashMap<String, FieldKind> {
    fn kind_of(&self, field_name: &str) -> Option<FieldKind> {
        self.get(field_name).copied()
    }
}

/// A record's fields, read according to the kinds a model declares.
pub struct MapOfFieldsSeed<'a, K: FieldKinds + ?Sized>(pub &'a K);

impl<'de, K: FieldKinds + ?Sized> DeserializeSeed<'de> for MapOfFieldsSeed<'_, K> {
    type Value = MapOfFields;

    fn deserialize<D>(self, deserializer: D) -> Result<MapOfFields, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(MapOfFieldsVisitor {
            kinds: self.0,
            takes_id: false,
        })
    }
}

pub(crate) struct MapOfFieldsVisitor<'a, K: FieldKinds + ?Sized> {
    kinds: &'a K,
    takes_id: bool,
}

impl<'a, K: FieldKinds + ?Sized> MapOfFieldsVisitor<'a, K> {
    /// Read a record that may name itself by an `id`, kept apart from its values.
    pub(crate) fn with_id(kinds: &'a K) -> Self {
        MapOfFieldsVisitor {
            kinds,
            takes_id: true,
        }
    }

    pub(crate) fn without_id(kinds: &'a K) -> Self {
        MapOfFieldsVisitor {
            kinds,
            takes_id: false,
        }
    }

    pub(crate) fn read<'de, M>(self, map: M) -> Result<(Option<u32>, MapOfFields), M::Error>
    where
        M: MapAccess<'de>,
    {
        self.read_from(None, map)
    }

    /// The same, the first key already taken off the map.
    pub(crate) fn read_from<'de, M>(
        self,
        first: Option<String>,
        mut map: M,
    ) -> Result<(Option<u32>, MapOfFields), M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut id = None;
        let mut fields = MapOfFields::default();
        let mut next = first;
        if next.is_none() {
            next = map.next_key::<String>()?;
        }
        while let Some(name) = next.take() {
            if self.takes_id && name == "id" {
                id = Some(map.next_value::<u32>()?);
                next = map.next_key::<String>()?;
                continue;
            }
            // An unknown field is refused rather than dropped: over the wire, silently ignoring
            // one turns a typo into a write that appears to have worked.
            let kind = self.kinds.kind_of(&name).ok_or_else(|| {
                Error::custom(format!("Field \"{name}\" is not declared on this model"))
            })?;
            let target = self.kinds.target(&name);
            let value = map.next_value_seed(FieldValue {
                kind,
                target: target.as_deref(),
            })?;
            fields.fields.insert(name, value);
            next = map.next_key::<String>()?;
        }
        Ok((id, fields))
    }
}

impl<'de, K: FieldKinds + ?Sized> Visitor<'de> for MapOfFieldsVisitor<'_, K> {
    type Value = MapOfFields;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map of field names to values")
    }

    fn visit_map<M>(self, map: M) -> Result<MapOfFields, M::Error>
    where
        M: MapAccess<'de>,
    {
        self.read(map).map(|(_, fields)| fields)
    }
}

/// A field's value, or nothing: for a one2many or a many2many whose target is known, commands
/// — read as plain ids when they are only that.
struct FieldValue<'a> {
    kind: FieldKind,
    target: Option<&'a dyn FieldKinds>,
}

impl<'de> DeserializeSeed<'de> for FieldValue<'_> {
    type Value = Option<FieldType>;

    fn deserialize<D>(self, deserializer: D) -> Result<Option<FieldType>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match (self.kind, self.target) {
            (FieldKind::Refs, Some(target)) => Ok(deserializer
                .deserialize_option(OptionalRelation(target))?
                .map(|value| match value {
                    RelationValue::Ids(ids) => FieldType::Refs(ids),
                    RelationValue::Commands(commands) => FieldType::Commands(commands),
                })),
            (kind, _) => OptionalValue(kind).deserialize(deserializer),
        }
    }
}

struct OptionalRelation<'a>(&'a dyn FieldKinds);

impl<'de> Visitor<'de> for OptionalRelation<'_> {
    type Value = Option<RelationValue>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a list of ids, commands, or null")
    }

    fn visit_none<E: Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        RelationValueSeed(self.0)
            .deserialize(deserializer)
            .map(Some)
    }
}
