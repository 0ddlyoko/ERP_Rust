use crate::{InvalidDomainError, LeftTuple, RightTuple, SearchKey, SearchOperator, SearchTuple, UnknownSearchOperatorError};
use std::error;
use std::fmt::Display;

#[derive(Clone, PartialEq, Debug)]
pub enum SearchType {
    And(Box<SearchType>, Box<SearchType>),
    Or(Box<SearchType>, Box<SearchType>),
    Tuple(SearchTuple),
    Nothing,
}

impl SearchType {
    /// Retrieve fields in "left" operator of all Tuple related to this field
    pub fn get_fields(&self) -> Vec<&LeftTuple> {
        let mut result: Vec<&LeftTuple> = vec![];
        self.handle_search_type(&mut result);

        result
    }

    fn handle_search_type<'a>(&'a self, result: &mut Vec<&'a LeftTuple>)
    {
        match self {
            SearchType::And(left, right) | SearchType::Or(left, right) => {
                SearchType::handle_search_type(left, result);
                SearchType::handle_search_type(right, result);
            },
            SearchType::Tuple(tuple) => {
                result.push(&tuple.left);
            },
            SearchType::Nothing => {},
        }
    }
}

impl From<SearchTuple> for SearchType
{
    fn from(search_type: SearchTuple) -> Self {
        SearchType::Tuple(search_type)
    }
}

impl<L, OP, R> TryFrom<(L, OP, R)> for SearchType
where
    L: Into<LeftTuple>,
    OP: TryInto<SearchOperator, Error = UnknownSearchOperatorError>,
    R: Into<RightTuple>
{
    type Error = UnknownSearchOperatorError;

    fn try_from(value: (L, OP, R)) -> Result<Self, Self::Error> {
        Ok(SearchType::Tuple(value.try_into()?))
    }
}

#[derive(Debug, Clone)]
pub enum ErrorType {
    InvalidDomain(InvalidDomainError),
    UnknownSearchOperator(UnknownSearchOperatorError),
}

impl Display for ErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorType::InvalidDomain(e) => e.fmt(f),
            ErrorType::UnknownSearchOperator(e) => e.fmt(f),
        }
    }
}

impl error::Error for ErrorType {}



impl TryFrom<Vec<SearchKey>> for SearchType {
    type Error = ErrorType;

    fn try_from(mut value: Vec<SearchKey>) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Ok(SearchType::Nothing);
        }

        /// Transform given value to a SearchType if possible, and return the rest of the list that
        ///  hasn't been parsed.
        ///
        /// If one element of the list is not transformable into a SearchType, return None
        fn parse_value(value: &mut Vec<SearchKey>) -> Option<SearchType> {
            if value.is_empty() {
                return Some(SearchType::Nothing);
            }
            let search_key = value.remove(0);
            match search_key {
                SearchKey::And | SearchKey::Or => {
                    let left_value = parse_value(value);
                    if let Some(left_search_type) = left_value {
                        // TODO Check if it's possible that "Nothing" is returned here
                        if left_search_type == SearchType::Nothing {
                            return None;
                        }
                        let right_value = parse_value(value);
                        if let Some(right_search_type) = right_value {
                            // TODO Check if it's possible that "Nothing" is returned here
                            if right_search_type == SearchType::Nothing {
                                return None;
                            }
                            return Some(if search_key == SearchKey::And {
                                SearchType::And(Box::new(left_search_type), Box::new(right_search_type))
                            } else {
                                SearchType::Or(Box::new(left_search_type), Box::new(right_search_type))
                            })
                        }
                    }
                    None
                },
                SearchKey::Tuple(tuple) => Some(SearchType::Tuple(tuple)),
            }
        }

        let original_value = value.clone();
        let result = parse_value(&mut value);
        if result.is_none() {
            return Err(ErrorType::InvalidDomain(InvalidDomainError { search_key: original_value }));
        }
        let mut result = result.unwrap();
        loop {
            if value.is_empty() {
                break;
            }
            let new_result = parse_value(&mut value);
            if new_result.is_none() {
                return Err(ErrorType::InvalidDomain(InvalidDomainError { search_key: original_value }));
            }
            let new_result = new_result.unwrap();
            result = SearchType::And(Box::new(result), Box::new(new_result));
        }

        Ok(result)
    }
}

impl<E> TryFrom<Vec<E>> for SearchType
where
    E: TryInto<SearchKey, Error = UnknownSearchOperatorError>,
{
    type Error = ErrorType;

    fn try_from(value: Vec<E>) -> Result<Self, Self::Error> {
        let result_values: Vec<Result<SearchKey, _>> = value.into_iter().map(|val| val.try_into()).collect();
        for val in &result_values {
            if let Err(err) = val {
                return Err(ErrorType::UnknownSearchOperator(err.clone()));
            }
        }

        let value: Vec<SearchKey> = result_values.into_iter().map(|val| val.unwrap()).collect();

        value.try_into()
    }
}
