//! Reading a domain off the wire.
//!
//! The form is the one the macro already speaks, in prefix notation:
//!
//! ```json
//! [["state", "=", "draft"]]
//! ["|", ["state", "=", "draft"], ["amount", ">", 100]]
//! ```
//!
//! A bare list of conditions is an implicit AND, and an empty list selects everything. The
//! folding is not reimplemented here: the elements are read into [`SearchKey`]s and handed to the
//! conversion the macro's output already goes through, so both spellings of a domain can never
//! drift apart.

use crate::{LeftTuple, RightTuple, SearchKey, SearchOperator, SearchTuple, SearchType};
use serde::Deserialize;
use serde::de::{Deserializer, Error, SeqAccess, Visitor};
use std::fmt;

impl<'de> Deserialize<'de> for SearchType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let keys = Vec::<SearchKey>::deserialize(deserializer)?;
        SearchType::try_from(keys).map_err(Error::custom)
    }
}

impl<'de> Deserialize<'de> for SearchKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(SearchKeyVisitor)
    }
}

struct SearchKeyVisitor;

impl<'de> Visitor<'de> for SearchKeyVisitor {
    type Value = SearchKey;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a condition [field, operator, value], or \"&\" or \"|\"")
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<SearchKey, E> {
        SearchKey::try_from(value).map_err(Error::custom)
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<SearchKey, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let field: String = seq
            .next_element()?
            .ok_or_else(|| Error::custom("a condition needs a field name"))?;
        let operator: String = seq
            .next_element()?
            .ok_or_else(|| Error::custom("a condition needs an operator"))?;
        let right: RightTuple = seq
            .next_element()?
            .ok_or_else(|| Error::custom("a condition needs a value"))?;
        if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
            return Err(Error::custom(
                "a condition is exactly [field, operator, value]",
            ));
        }
        let operator = SearchOperator::try_from(operator.as_str()).map_err(Error::custom)?;
        Ok(SearchKey::Tuple(SearchTuple {
            left: LeftTuple::from(field),
            operator,
            right,
        }))
    }
}

impl<'de> Deserialize<'de> for RightTuple {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(RightTupleVisitor)
    }
}

/// The right-hand side, read as it arrives.
///
/// Untyped on purpose: what a value means is decided by the column it is compared against, and
/// the path on the left may cross several models before naming one. A date or a decimal
/// therefore arrives as the string it was written as, exactly as it does from a Rust caller
/// writing `("due_date", "=", "2026-09-30")`.
struct RightTupleVisitor;

impl<'de> Visitor<'de> for RightTupleVisitor {
    type Value = RightTuple;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string, a number, a boolean, null, or a list of those")
    }

    fn visit_str<E: Error>(self, value: &str) -> Result<RightTuple, E> {
        Ok(RightTuple::String(value.to_string()))
    }

    fn visit_bool<E: Error>(self, value: bool) -> Result<RightTuple, E> {
        Ok(RightTuple::Boolean(value))
    }

    fn visit_i64<E: Error>(self, value: i64) -> Result<RightTuple, E> {
        i32::try_from(value)
            .map(RightTuple::Integer)
            .map_err(|_| Error::custom(format!("{value} does not fit in a 32-bit integer")))
    }

    fn visit_u64<E: Error>(self, value: u64) -> Result<RightTuple, E> {
        u32::try_from(value)
            .map(RightTuple::UInteger)
            .map_err(|_| Error::custom(format!("{value} does not fit in a 32-bit integer")))
    }

    fn visit_f64<E: Error>(self, value: f64) -> Result<RightTuple, E> {
        Err(Error::custom(format!(
            "{value} is a fraction, which cannot be compared exactly. Send it as a string instead."
        )))
    }

    fn visit_none<E: Error>(self) -> Result<RightTuple, E> {
        Ok(RightTuple::None)
    }

    fn visit_unit<E: Error>(self) -> Result<RightTuple, E> {
        Ok(RightTuple::None)
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<RightTuple, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element::<RightTuple>()? {
            values.push(value);
        }
        Ok(RightTuple::Array(values))
    }
}
