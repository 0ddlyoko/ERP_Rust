use chrono::{DateTime, NaiveDate, Utc};
use erp_search::RightTuple;
use erp_types::field as field_type;
use erp_types::field::{Password, StoredHash};
use rust_decimal::Decimal;
use std::fmt::{Display, Formatter};

#[macro_export]
macro_rules! database_field_type_make_eq {
    ( $self:expr, $other:expr, $( $path:path ),* ) => {
        match $self {
            $($path(self_value) => {
                if let $path(other_value) = $other {
                    self_value == other_value
                } else {
                    false
                }
            })*
        }
    };
}

#[derive(Debug, Clone)]
pub enum FieldType {
    String(String),
    Integer(i32),
    UInteger(u32),
    Decimal(Decimal),
    Boolean(bool),
    Date(NaiveDate),
    DateTime(DateTime<Utc>),
    /// A password, as the hash that is what actually reaches a column.
    Password(String),
}

impl Display for FieldType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::String(s) => write!(f, "{}", s),
            FieldType::Integer(i) => write!(f, "{}", i),
            FieldType::UInteger(b) => write!(f, "{}", b),
            FieldType::Decimal(d) => write!(f, "{}", d),
            FieldType::Boolean(e) => write!(f, "{}", e),
            FieldType::Date(d) => write!(f, "{}", d),
            FieldType::DateTime(dt) => write!(f, "{}", dt),
            FieldType::Password(_) => write!(f, "****"),
        }
    }
}

impl PartialEq for FieldType {
    fn eq(&self, other: &Self) -> bool {
        database_field_type_make_eq!(
            self,
            other,
            FieldType::String,
            FieldType::Integer,
            FieldType::UInteger,
            FieldType::Decimal,
            FieldType::Boolean,
            FieldType::Date,
            FieldType::DateTime,
            FieldType::Password
        )
    }
}

impl From<FieldType> for RightTuple {
    fn from(other: FieldType) -> Self {
        match other {
            FieldType::String(value) => RightTuple::String(value),
            FieldType::Integer(value) => RightTuple::Integer(value),
            FieldType::UInteger(value) => RightTuple::UInteger(value),
            FieldType::Decimal(value) => RightTuple::Decimal(value),
            FieldType::Boolean(value) => RightTuple::Boolean(value),
            FieldType::Date(value) => RightTuple::Date(value),
            FieldType::DateTime(value) => RightTuple::DateTime(value),
            // Nothing a domain could have written, which is the point: a password has no
            // comparable form.
            FieldType::Password(_) => RightTuple::None,
        }
    }
}

impl PartialEq<RightTuple> for FieldType {
    fn eq(&self, other: &RightTuple) -> bool {
        match (self, other) {
            (FieldType::String(value), RightTuple::String(other_value)) => value == other_value,
            (FieldType::Integer(value), RightTuple::Integer(other_value)) => value == other_value,
            (FieldType::UInteger(value), RightTuple::UInteger(other_value)) => value == other_value,
            (FieldType::Integer(value), RightTuple::UInteger(other_value)) => {
                i64::from(*value) == i64::from(*other_value)
            }
            (FieldType::UInteger(value), RightTuple::Integer(other_value)) => {
                i64::from(*value) == i64::from(*other_value)
            }
            (FieldType::Decimal(value), RightTuple::Decimal(other_value)) => value == other_value,
            (FieldType::Boolean(value), RightTuple::Boolean(other_value)) => value == other_value,
            (FieldType::Date(value), RightTuple::Date(other_value)) => value == other_value,
            (FieldType::DateTime(value), RightTuple::DateTime(other_value)) => value == other_value,
            // Before the arms that compare: a password matches no value at all, not even one
            // that happens to be its hash. Answering would turn a domain into an oracle that
            // reconstructs the hash without ever reading it.
            (FieldType::Password(_), _) => false,
            (value, RightTuple::Array(other_value)) => other_value.contains(&value.clone().into()),
            _ => false,
        }
    }
}

impl PartialEq<FieldType> for RightTuple {
    fn eq(&self, other: &FieldType) -> bool {
        // Call method above
        other.eq(self)
    }
}

impl From<field_type::FieldType> for FieldType {
    fn from(value: field_type::FieldType) -> Self {
        match value {
            field_type::FieldType::String(v) => FieldType::String(v),
            field_type::FieldType::Integer(v) => FieldType::Integer(v),
            field_type::FieldType::Decimal(v) => FieldType::Decimal(v),
            field_type::FieldType::Bool(v) => FieldType::Boolean(v),
            field_type::FieldType::Date(v) => FieldType::Date(v),
            field_type::FieldType::DateTime(v) => FieldType::DateTime(v),
            field_type::FieldType::Ref(v) => FieldType::UInteger(v),
            // Taken by value, which is what lets the hash out at all — see `Password::into_hash`.
            field_type::FieldType::Password(v) => FieldType::Password(v.into_hash()),
            // Never reaches a column: a list of references has none, and commands are turned
            // into ids before anything is saved.
            field_type::FieldType::Refs(_) | field_type::FieldType::Commands(_) => {
                panic!("Cannot convert Refs fields to database objet")
            }
        }
    }
}

impl From<FieldType> for field_type::FieldType {
    fn from(value: FieldType) -> Self {
        match value {
            FieldType::String(v) => field_type::FieldType::String(v),
            FieldType::Integer(v) => field_type::FieldType::Integer(v),
            FieldType::UInteger(v) => field_type::FieldType::Ref(v),
            FieldType::Decimal(v) => field_type::FieldType::Decimal(v),
            FieldType::Boolean(v) => field_type::FieldType::Bool(v),
            FieldType::Date(v) => field_type::FieldType::Date(v),
            FieldType::DateTime(v) => field_type::FieldType::DateTime(v),
            FieldType::Password(v) => field_type::FieldType::Password(Password::from_hash(v)),
        }
    }
}

/// `false` compared with a field that is no yes-or-no is its being empty, in a domain written
/// in Rust as in one sent by a client: `("sale_line", "!=", false)` finds the records with a line.
pub(crate) fn empty_for_false(
    kind: erp_types::field::FieldKind,
    right: &RightTuple,
) -> std::borrow::Cow<'_, RightTuple> {
    match right {
        RightTuple::Boolean(false) if kind != erp_types::field::FieldKind::Bool => {
            std::borrow::Cow::Owned(RightTuple::None)
        }
        right => std::borrow::Cow::Borrowed(right),
    }
}
