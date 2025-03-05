use std::{error, fmt};
use crate::search::SearchKey;

#[derive(Debug, Clone)]
pub struct InvalidDomainError {
    pub(crate) search_key: Vec<SearchKey>,
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
