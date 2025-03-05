use crate::search::SearchTuple;

#[derive(Clone, PartialEq, Debug)]
pub enum SearchKey {
    And,
    Or,
    Tuple(SearchTuple),
    Unknown,
}

impl From<&str> for SearchKey {
    fn from(value: &str) -> Self {
        match value {
            "&" => SearchKey::And,
            "and" => SearchKey::And,
            "|" => SearchKey::Or,
            "or" => SearchKey::Or,
            _ => SearchKey::Unknown,
        }
    }
}

impl From<String> for SearchKey {
    fn from(value: String) -> Self {
        value.as_str().into()
    }
}

impl From<&String> for SearchKey {
    fn from(value: &String) -> Self {
        value.as_str().into()
    }
}

impl<E> From<E> for SearchKey
where
    SearchTuple: From<E>
{
    fn from(value: E) -> Self {
        SearchKey::Tuple(value.into())
    }
}
