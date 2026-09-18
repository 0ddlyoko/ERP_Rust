use crate::UnknownSearchOperatorError;

#[derive(Clone, PartialEq, Debug)]
pub enum SearchOperator {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Lower,
    LowerEqual,
    /// Case-sensitive pattern match, with SQL wildcards: `%` for any run, `_` for one character.
    Like,
    /// Case-insensitive [`SearchOperator::Like`].
    ILike,
    /// Membership. `Equal` against an array means the same thing and stays valid; this spelling
    /// is the explicit one, and rejects a right-hand side that is not an array.
    In,
    /// Negated [`SearchOperator::In`].
    NotIn,
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
            "like" => SearchOperator::Like,
            "ilike" => SearchOperator::ILike,
            "in" => SearchOperator::In,
            "not in" => SearchOperator::NotIn,
            _ => {
                return Err(UnknownSearchOperatorError {
                    search_operator: str.to_string(),
                });
            }
        })
    }
}

impl TryFrom<String> for SearchOperator {
    type Error = UnknownSearchOperatorError;

    fn try_from(str: String) -> Result<Self, UnknownSearchOperatorError> {
        str.as_str().try_into()
    }
}
