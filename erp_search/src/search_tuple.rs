use crate::{SearchOperator, UnknownSearchOperatorError};

#[derive(Clone, PartialEq, Debug)]
pub struct SearchTuple {
    pub left: String,
    pub operator: SearchOperator,
    // TODO Right tuple isn't always a string, it could be a bool, and int, a list or a date(time)
    //  Fix this by using an enum
    pub right: String,
}

impl<E> TryFrom<(&str, E, &str)> for SearchTuple
where
    E: TryInto<SearchOperator, Error = UnknownSearchOperatorError>,
{
    type Error = UnknownSearchOperatorError;

    fn try_from(tuple: (&str, E, &str)) -> Result<Self, Self::Error> {
        Ok(Self {
            left: tuple.0.to_string(),
            operator: tuple.1.try_into()?,
            right: tuple.2.to_string(),
        })
    }
}

impl<E> TryFrom<(String, E, String)> for SearchTuple
where
    E: TryInto<SearchOperator, Error = UnknownSearchOperatorError>,
{
    type Error = UnknownSearchOperatorError;

    fn try_from(tuple: (String, E, String)) -> Result<Self, Self::Error> {
        Ok(Self {
            left: tuple.0,
            operator: tuple.1.try_into()?,
            right: tuple.2,
        })
    }
}
