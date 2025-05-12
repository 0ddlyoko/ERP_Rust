use crate::{SearchOperator, UnknownSearchOperatorError};

#[derive(Clone, PartialEq, Debug)]
pub struct SearchTuple {
    pub left: String,
    pub operator: SearchOperator,
    pub right: RightTuple,
}

impl<E, F> TryFrom<(&str, E, F)> for SearchTuple
where
    E: TryInto<SearchOperator, Error = UnknownSearchOperatorError>,
    F: Into<RightTuple>
{
    type Error = UnknownSearchOperatorError;

    fn try_from(tuple: (&str, E, F)) -> Result<Self, Self::Error> {
        Ok(Self {
            left: tuple.0.to_string(),
            operator: tuple.1.try_into()?,
            right: tuple.2.into(),
        })
    }
}

impl<E, F> TryFrom<(String, E, F)> for SearchTuple
where
    E: TryInto<SearchOperator, Error = UnknownSearchOperatorError>,
    F: Into<RightTuple>
{
    type Error = UnknownSearchOperatorError;

    fn try_from(tuple: (String, E, F)) -> Result<Self, Self::Error> {
        Ok(Self {
            left: tuple.0,
            operator: tuple.1.try_into()?,
            right: tuple.2.into(),
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum RightTuple {
    String(String),
    Integer(i32),
    UInteger(u32),
    Float(f32),
    Boolean(bool),
    Array(Vec<RightTuple>),
    None,
}

impl From<&str> for RightTuple {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<String> for RightTuple {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<i32> for RightTuple {
    fn from(i: i32) -> Self {
        Self::Integer(i)
    }
}

impl From<u32> for RightTuple {
    fn from(f: u32) -> Self {
        Self::UInteger(f)
    }
}

impl From<f32> for RightTuple {
    fn from(f: f32) -> Self {
        Self::Float(f)
    }
}

impl From<bool> for RightTuple {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}

impl From<Vec<RightTuple>> for RightTuple {
    fn from(v: Vec<RightTuple>) -> Self {
        Self::Array(v)
    }
}

impl From<Option<RightTuple>> for RightTuple {
    fn from(v: Option<RightTuple>) -> Self {
        v.unwrap_or_else(|| Self::None)
    }
}
