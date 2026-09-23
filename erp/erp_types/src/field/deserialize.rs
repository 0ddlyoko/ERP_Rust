//! Reading field values off the wire.
//!
//! A value arrives bare — `7`, `"0ddlyoko"`, `[1, 2]` — because that is how it goes out. Nothing
//! in it says whether `7` is an integer or the id of a related record, so deserialization is
//! driven by the kind the model declares, never by the shape of the incoming value.
//!
//! Expressed as [`serde::de::DeserializeSeed`] rather than a function over a parsed JSON tree:
//! the seed carries the kind, works with any format, and keeps this crate free of a dependency on
//! one.

use crate::field::{FieldKind, FieldType, Password};
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
        deserializer.deserialize_map(MapOfFieldsVisitor(self.0))
    }
}

struct MapOfFieldsVisitor<'a, K: FieldKinds + ?Sized>(&'a K);

impl<'de, K: FieldKinds + ?Sized> Visitor<'de> for MapOfFieldsVisitor<'_, K> {
    type Value = MapOfFields;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map of field names to values")
    }

    fn visit_map<M>(self, mut map: M) -> Result<MapOfFields, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut fields = MapOfFields::default();
        while let Some(name) = map.next_key::<String>()? {
            // An unknown field is refused rather than dropped: over the wire, silently ignoring
            // one turns a typo into a write that appears to have worked.
            let kind = self.0.kind_of(&name).ok_or_else(|| {
                Error::custom(format!("Field \"{name}\" is not declared on this model"))
            })?;
            let value = map.next_value_seed(OptionalValue(kind))?;
            fields.fields.insert(name, value);
        }
        Ok(fields)
    }
}
