use crate::field::{FieldKind, FieldType};
use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::sync::{LazyLock, Mutex};

/// Where a value declared by an enum goes among the values of its family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Where it already is; a new value goes last.
    Unchanged,
    After(&'static str),
    Before(&'static str),
}

/// One value an enum declares, as `#[selection]` wrote it down.
#[derive(Debug, Clone, Copy)]
pub struct SelectionValue {
    pub key: &'static str,
    pub label: &'static str,
    /// Whether the label was written, rather than made from the variant's name: only a written
    /// one replaces the label of a value another enum declared.
    pub label_given: bool,
    pub placement: Placement,
}

/// The key a value is stored under.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SelectionKey(&'static str);

impl SelectionKey {
    /// A key written in the code.
    pub const fn from_static(key: &'static str) -> Self {
        SelectionKey(key)
    }

    /// A key read at run time, kept for the rest of the process: keys are few, and an enum
    /// value must be `Copy`.
    pub fn new(key: &str) -> Self {
        static KEYS: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Default::default);
        let mut keys = KEYS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(known) = keys.get(key) {
            return SelectionKey(known);
        }
        let leaked: &'static str = Box::leak(key.to_string().into_boxed_str());
        keys.insert(leaked);
        SelectionKey(leaked)
    }

    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

impl Debug for SelectionKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Display for SelectionKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialEq<&str> for SelectionKey {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// An enum declared with `#[selection]`: a set of values stored by key, which other enums of the
/// same family may name, add to, move or relabel.
///
/// Every enum of a family names the same keys: `A::B` and `D::B` are the same value. Values an
/// enum does not name are its `Extended` variant.
pub trait Selection: Copy + Eq + Debug + 'static {
    /// The enum the family started from.
    type Root: Selection;
    /// The enum this one extends; the root extends itself.
    type Parent: Selection;
    /// Names the family: the root enum's path.
    const FAMILY: &'static str;
    /// What this enum declares, in order.
    const VALUES: &'static [SelectionValue];

    fn key(&self) -> SelectionKey;

    /// The value under a key; one this enum does not name is `Extended`.
    fn from_key(key: &str) -> Self;

    /// The same, as a reference that outlives the value it was read from, for getters.
    fn from_key_ref(key: &str) -> &'static Self;

    /// No value, as a field of this enum reads on an empty record.
    fn empty_ref() -> &'static Self;

    /// Whether both are the same value, whichever enums of the family name them.
    fn is<T: Selection<Root = Self::Root>>(&self, other: T) -> bool {
        self.key() == other.key()
    }

    /// The same value, as another enum of the family names it.
    fn to<T: Selection<Root = Self::Root>>(self) -> T {
        T::from_key(self.key().as_str())
    }
}

/// A family of values, as a field declared with one of its enums knows it.
#[derive(Debug, Clone, Copy)]
pub struct SelectionFamily {
    pub family: &'static str,
    /// What the root enum declares, to start the family from.
    pub root_values: &'static [SelectionValue],
}

impl SelectionFamily {
    pub fn of<E: Selection>() -> Self {
        SelectionFamily {
            family: E::FAMILY,
            root_values: <E::Root as Selection>::VALUES,
        }
    }
}

/// What a required field's setter takes: a value of its type, or for an enum, a value of any
/// enum of its family.
pub trait Accepts<V> {
    fn accept(value: V) -> Self;
}

macro_rules! accepts_itself {
    ( $( $ty:ty ),* ) => {
        $( impl Accepts<$ty> for $ty {
            fn accept(value: $ty) -> Self {
                value
            }
        } )*
    };
}

accepts_itself!(
    String,
    i32,
    u32,
    bool,
    rust_decimal::Decimal,
    chrono::NaiveDate,
    chrono::DateTime<chrono::Utc>,
    crate::field::Password
);

impl Accepts<&str> for String {
    fn accept(value: &str) -> Self {
        value.to_string()
    }
}

/// What an optional field's setter takes: the value alone, `Some` of it, or `None` to empty it.
pub trait AcceptsOptional<V>: Sized {
    fn accept_optional(value: V) -> Option<Self>;
}

impl<T> AcceptsOptional<Option<T>> for T {
    fn accept_optional(value: Option<T>) -> Option<Self> {
        value
    }
}

macro_rules! accepts_itself_optionally {
    ( $( $ty:ty ),* ) => {
        $( impl AcceptsOptional<$ty> for $ty {
            fn accept_optional(value: $ty) -> Option<Self> {
                Some(value)
            }
        } )*
    };
}

accepts_itself_optionally!(
    String,
    i32,
    u32,
    bool,
    rust_decimal::Decimal,
    chrono::NaiveDate,
    chrono::DateTime<chrono::Utc>,
    crate::field::Password
);

impl AcceptsOptional<&str> for String {
    fn accept_optional(value: &str) -> Option<Self> {
        Some(value.to_string())
    }
}

impl AcceptsOptional<Option<&str>> for String {
    fn accept_optional(value: Option<&str>) -> Option<Self> {
        value.map(str::to_string)
    }
}

impl<E: Selection> AcceptsOptional<E> for E {
    fn accept_optional(value: E) -> Option<Self> {
        Some(value)
    }
}

impl<E: Selection> From<E> for FieldType {
    fn from(value: E) -> Self {
        FieldType::String(value.key().as_str().to_string())
    }
}

impl<'a, E: Selection> From<&'a FieldType> for Option<&'a E> {
    fn from(value: &'a FieldType) -> Self {
        match value {
            FieldType::String(key) => Some(E::from_key_ref(key)),
            _ => None,
        }
    }
}

/// Tells generated code what a field's Rust type is stored as, and whether it is an enum.
///
/// Called as `(&&FieldProbe::<T>::new()).describe()`: an enum is found through
/// [`SelectionFieldProbe`], anything else falls back to [`PlainFieldProbe`]. The macro cannot
/// tell an enum from a struct by its name.
pub struct FieldProbe<T>(PhantomData<T>);

impl<T> FieldProbe<T> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        FieldProbe(PhantomData)
    }
}

pub trait SelectionFieldProbe {
    fn describe(&self) -> (FieldKind, Option<SelectionFamily>);
}

impl<T: Selection> SelectionFieldProbe for &FieldProbe<T> {
    fn describe(&self) -> (FieldKind, Option<SelectionFamily>) {
        (FieldKind::String, Some(SelectionFamily::of::<T>()))
    }
}

pub trait PlainFieldProbe {
    fn describe(&self) -> (FieldKind, Option<SelectionFamily>);
}

impl<T: Default + Into<FieldType>> PlainFieldProbe for FieldProbe<T> {
    fn describe(&self) -> (FieldKind, Option<SelectionFamily>) {
        (Into::<FieldType>::into(T::default()).kind(), None)
    }
}
