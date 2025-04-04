use crate::search::{InvalidDomainError, SearchKey, SearchTuple};

// #[macro_export]
// macro_rules! domain {
//     // Empty
//     ([]) => {
//         Vec::<SearchKey>::new()
//     };
//
//     // Single expression
//     (($left:expr, $op:expr, $right:expr)) => {
//         SearchTuple::from(($left, $op, $right))
//     };
//
//     // AND expression
//     ("&", $($rest:tt),+) => {
//         let mut vec = vec![SearchKey::And];
//         vec.extend(vec![$(domain!($rest)),+]);
//         vec
//     };
//
//     // OR expression
//     ("|", $($rest:tt),+) => {
//         let mut vec = vec![SearchKey::Or];
//         vec.extend(vec![$(domain!($rest)),+]);
//         vec
//     };
//
//     // Multiple expression, first is tuple
//
//
//     // Multiple expression
//     ([$first:tt, $($rest:tt),+]) => {
//     }
// }

#[derive(Clone, PartialEq, Debug)]
pub enum SearchType {
    And(Box<SearchType>, Box<SearchType>),
    Or(Box<SearchType>, Box<SearchType>),
    Tuple(SearchTuple),
    Nothing,
}

impl<E> From<E> for SearchType
where
    SearchTuple: From<E>,
{
    fn from(value: E) -> Self {
        SearchType::Tuple(value.into())
    }
}

impl<E> TryFrom<Vec<E>> for SearchType
where
    SearchKey: From<E>,
    E: Clone
{
    type Error = InvalidDomainError;

    fn try_from(value: Vec<E>) -> Result<Self, Self::Error> {
        let value: Vec<SearchKey> = value.into_iter().map(SearchKey::from).collect();
        // Pre-check: If "Unknown" is part of "value", then there the domain is invalid
        if value.contains(&SearchKey::Unknown) {
            return Err(InvalidDomainError { search_key: value });
        }

        /// Transform given value to a SearchType if possible, and return the rest of the list that
        ///  hasn't been parsed.
        ///
        /// If one element of the list is not transformable into a SearchType, return None
        fn parse_value(mut value: Vec<SearchKey>) -> Option<(SearchType, Vec<SearchKey>)> {
            if value.is_empty() {
                return Some((SearchType::Nothing, value));
            }
            let search_key = value.remove(0);
            match search_key {
                SearchKey::And | SearchKey::Or => {
                    let left_value = parse_value(value);
                    if let Some((left_search_type, value)) = left_value {
                        // TODO Check if it's possible that "Nothing" is returned here
                        if left_search_type == SearchType::Nothing {
                            return None;
                        }
                        if value.is_empty() {
                            return None;
                        }
                        let right_value = parse_value(value);
                        if let Some((right_search_type, value)) = right_value {
                            // TODO Check if it's possible that "Nothing" is returned here
                            if right_search_type == SearchType::Nothing {
                                return None;
                            }
                            return Some((if search_key == SearchKey::And {
                                SearchType::And(Box::new(left_search_type), Box::new(right_search_type))
                            } else {
                                SearchType::Or(Box::new(left_search_type), Box::new(right_search_type))
                            }, value))
                        }
                    }
                    None
                },
                SearchKey::Tuple(tuple) => Some((SearchType::Tuple(tuple), value)),
                SearchKey::Unknown => None,
            }
        }

        let original_value = value.clone();
        let result = parse_value(value);
        if result.is_none() {
            return Err(InvalidDomainError { search_key: original_value });
        }
        let mut result = result.unwrap();
        loop {
            if result.1.is_empty() {
                break;
            }
            let new_result = parse_value(result.1);
            if new_result.is_none() {
                return Err(InvalidDomainError { search_key: original_value });
            }
            let new_result = new_result.unwrap();
            result = (SearchType::And(Box::new(result.0), Box::new(new_result.0)), new_result.1);
        }

        Ok(result.0)
    }
}
