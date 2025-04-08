use crate::{SearchTuple, UnknownSearchKeyError, UnknownSearchOperatorError};

#[derive(Clone, PartialEq, Debug)]
pub enum SearchKey {
    And,
    Or,
    Tuple(SearchTuple),
}

impl TryFrom<&str> for SearchKey {
    type Error = UnknownSearchKeyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "&" => Ok(SearchKey::And),
            // "and" => SearchKey::And,
            "|" => Ok(SearchKey::Or),
            // "or" => SearchKey::Or,
            _ => Err(UnknownSearchKeyError { search_key: value.to_string() }),
        }
    }
}

impl TryFrom<String> for SearchKey {
    type Error = UnknownSearchKeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.as_str().try_into()
    }
}

impl TryFrom<&String> for SearchKey {
    type Error = UnknownSearchKeyError;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        value.as_str().try_into()
    }
}

// impl<E> TryFrom<E> for SearchKey doesn't work, and idk why.
// If it works again, we can replace the next 2 TryFrom impl.

impl<E> TryFrom<(&str, E, &str)> for SearchKey
where
    SearchTuple: for<'a> TryFrom<(&'a str, E, &'a str), Error = UnknownSearchOperatorError>,
{
    type Error = UnknownSearchOperatorError;

    fn try_from(value: (&str, E, &str)) -> Result<Self, Self::Error> {
        Ok(SearchKey::Tuple(value.try_into()?))
    }
}

impl<E> TryFrom<(String, E, String)> for SearchKey
where
    SearchTuple: TryFrom<(String, E, String), Error = UnknownSearchOperatorError>,
{
    type Error = UnknownSearchOperatorError;

    fn try_from(value: (String, E, String)) -> Result<Self, Self::Error> {
        Ok(SearchKey::Tuple(value.try_into()?))
    }
}
