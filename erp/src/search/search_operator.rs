
#[derive(Clone, PartialEq, Debug)]
pub enum SearchOperator {
    Equal,
    NotEqual,
    In,
    NotIn,
    Greater,
    GreaterEqual,
    Lower,
    LowerEqual,
}

impl From<&str> for SearchOperator {
    fn from(str: &str) -> Self {
        match str {
            "=" => SearchOperator::Equal,
            "!=" => SearchOperator::NotEqual,
            "in" => SearchOperator::In,
            "!in" => SearchOperator::NotIn,
            ">" => SearchOperator::Greater,
            ">=" => SearchOperator::GreaterEqual,
            "<" => SearchOperator::Lower,
            "<=" => SearchOperator::LowerEqual,
            _ => panic!("{}", format!("Invalid search type: {}", str)),
        }
    }
}
