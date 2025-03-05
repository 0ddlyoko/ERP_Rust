use crate::search::SearchOperator;

#[derive(Clone, PartialEq, Debug)]
pub struct SearchTuple {
    pub left: String,
    pub operator: SearchOperator,
    // TODO Right tuple isn't always a string, it could be a bool, and int, a list or a date(time)
    pub right: String,
}

impl<E> From<(&str, E, &str)> for SearchTuple
where
    SearchOperator: From<E>,
{
    fn from(tuple: (&str, E, &str)) -> Self {
        Self {
            left: tuple.0.to_string(),
            operator: tuple.1.into(),
            right: tuple.2.to_string(),
        }
    }
}

impl<E> From<(String, E, String)> for SearchTuple
where
    SearchOperator: From<E>,
{
    fn from(tuple: (String, E, String)) -> Self {
        Self {
            left: tuple.0,
            operator: tuple.1.into(),
            right: tuple.2,
        }
    }
}
