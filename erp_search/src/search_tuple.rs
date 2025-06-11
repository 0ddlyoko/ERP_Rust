use crate::{SearchOperator, UnknownSearchOperatorError};

#[derive(Clone, PartialEq, Debug)]
pub struct SearchTuple {
    pub left: LeftTuple,
    pub operator: SearchOperator,
    pub right: RightTuple,
}

impl<L, OP, R> TryFrom<(L, OP, R)> for SearchTuple
where
    L: Into<LeftTuple>,
    OP: TryInto<SearchOperator, Error = UnknownSearchOperatorError>,
    R: Into<RightTuple>
{
    type Error = UnknownSearchOperatorError;

    fn try_from(tuple: (L, OP, R)) -> Result<Self, Self::Error> {
        Ok(Self {
            left: tuple.0.into(),
            operator: tuple.1.try_into()?,
            right: tuple.2.into(),
        })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct LeftTuple {
    pub path: Vec<String>,
}

impl From<&str> for LeftTuple {
    fn from(value: &str) -> Self {
        let split = value.split(".").map(|str| str.to_string()).collect::<Vec<_>>();
        LeftTuple {
            path: split
        }
    }
}

impl From<&String> for LeftTuple {
    fn from(value: &String) -> Self {
        value.as_str().into()
    }
}

impl From<String> for LeftTuple {
    fn from(value: String) -> Self {
        value.as_str().into()
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

impl<E> From<Vec<E>> for RightTuple
where
    E: Into<RightTuple>
{
    fn from(v: Vec<E>) -> Self {
        Self::Array(v.into_iter().map(Into::into).collect())
    }
}

impl From<Option<RightTuple>> for RightTuple {
    fn from(v: Option<RightTuple>) -> Self {
        v.unwrap_or(Self::None)
    }
}
