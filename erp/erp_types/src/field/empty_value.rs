use crate::field::{Password, Selection};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use std::sync::OnceLock;

/// What a field reads as on an empty record: its type's default, kept for as long as the
/// program runs so a getter can hand out a reference to it.
pub trait EmptyValue: 'static {
    fn empty() -> &'static Self;
}

macro_rules! empty_by_default {
    ($($kind:ty),* $(,)?) => {
        $(
            impl EmptyValue for $kind {
                fn empty() -> &'static Self {
                    static EMPTY: OnceLock<$kind> = OnceLock::new();
                    EMPTY.get_or_init(<$kind>::default)
                }
            }
        )*
    };
}

empty_by_default!(
    String,
    i32,
    u32,
    bool,
    Decimal,
    NaiveDate,
    DateTime<Utc>,
    Password,
    Vec<u32>
);

impl<E: Selection> EmptyValue for E {
    fn empty() -> &'static Self {
        E::empty_ref()
    }
}
