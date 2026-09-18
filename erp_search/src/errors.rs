use crate::SearchKey;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("Given domain \"{search_key:?}\" is invalid, please check it")]
pub struct InvalidDomainError {
    pub search_key: Vec<SearchKey>,
}

#[derive(Debug, Clone, Error)]
#[error("Unknown search key \"{search_key:?}\", it should be \"&\" or \"|\"")]
pub struct UnknownSearchKeyError {
    pub search_key: String,
}

#[derive(Debug, Clone, Error)]
#[error(
    "Unknown search operator \"{search_operator:?}\", it should be \"=\", \"!=\", \">\", \">=\", \"<\" or \"<=\""
)]
pub struct UnknownSearchOperatorError {
    pub search_operator: String,
}
