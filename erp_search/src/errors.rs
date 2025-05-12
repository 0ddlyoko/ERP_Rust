use std::{error, fmt};
use crate::SearchKey;

#[derive(Debug, Clone)]
pub struct InvalidDomainError {
    pub search_key: Vec<SearchKey>,
}

impl fmt::Display for InvalidDomainError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Given domain \"{:?}\" is invalid, please check it",
            self.search_key
        )
    }
}

impl error::Error for InvalidDomainError {}

#[derive(Debug, Clone)]
pub struct UnknownSearchKeyError {
    pub search_key: String,
}

impl fmt::Display for UnknownSearchKeyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Unknown search key \"{:?}\", it should be \"&\" or \"|\"",
            self.search_key
        )
    }
}

impl error::Error for UnknownSearchKeyError {}

#[derive(Debug, Clone)]
pub struct UnknownSearchOperatorError {
    pub search_operator: String,
}

impl fmt::Display for UnknownSearchOperatorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Unknown search operator \"{:?}\", it should be \"=\", \"!=\", \">\", \">=\", \"<\" or \"<=\"",
            self.search_operator
        )
    }
}

impl error::Error for UnknownSearchOperatorError {}


