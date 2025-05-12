use crate::UnknownSearchOperatorError;

#[derive(Clone, PartialEq, Debug)]
pub enum SearchOperator {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Lower,
    LowerEqual,
}

impl TryFrom<&str> for SearchOperator {
    type Error = UnknownSearchOperatorError;

    fn try_from(str: &str) -> Result<Self, UnknownSearchOperatorError> {
        Ok(match str {
            "=" => SearchOperator::Equal,
            "!=" => SearchOperator::NotEqual,
            ">" => SearchOperator::Greater,
            ">=" => SearchOperator::GreaterEqual,
            "<" => SearchOperator::Lower,
            "<=" => SearchOperator::LowerEqual,
            _ => return Err(UnknownSearchOperatorError { search_operator: str.to_string() }),
        })
    }
}

impl TryFrom<String> for SearchOperator {
    type Error = UnknownSearchOperatorError;
    
    fn try_from(str: String) -> Result<Self, UnknownSearchOperatorError> {
        str.as_str().try_into()
    }
}
