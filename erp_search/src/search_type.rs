use std::collections::VecDeque;

use crate::{
    InvalidDomainError, LeftTuple, RightTuple, SearchKey, SearchOperator, SearchTuple,
    UnknownSearchOperatorError,
};

#[derive(Clone, PartialEq, Debug)]
pub enum SearchType {
    And(Box<SearchType>, Box<SearchType>),
    Or(Box<SearchType>, Box<SearchType>),
    Tuple(SearchTuple),
    /// No filter at all, which selects every record of the model.
    Nothing,
    /// A filter nothing satisfies, which selects none of them.
    ///
    /// The counterpart of [`SearchType::Nothing`], and not expressible as a comparison: a caller
    /// that must be answered "no records" without being told why needs to say so in the domain
    /// itself rather than through a condition on some field.
    Never,
}

impl SearchType {
    /// Retrieve fields in "left" operator of all Tuple related to this field
    pub fn get_fields(&self) -> Vec<&LeftTuple> {
        let mut result: Vec<&LeftTuple> = vec![];
        self.handle_search_type(&mut result);

        result
    }

    fn handle_search_type<'a>(&'a self, result: &mut Vec<&'a LeftTuple>) {
        match self {
            SearchType::And(left, right) | SearchType::Or(left, right) => {
                SearchType::handle_search_type(left, result);
                SearchType::handle_search_type(right, result);
            }
            SearchType::Tuple(tuple) => {
                result.push(&tuple.left);
            }
            SearchType::Nothing | SearchType::Never => {}
        }
    }
}

impl From<SearchTuple> for SearchType {
    fn from(search_type: SearchTuple) -> Self {
        SearchType::Tuple(search_type)
    }
}

impl<L, OP, R> TryFrom<(L, OP, R)> for SearchType
where
    L: Into<LeftTuple>,
    OP: TryInto<SearchOperator, Error = UnknownSearchOperatorError>,
    R: Into<RightTuple>,
{
    type Error = UnknownSearchOperatorError;

    fn try_from(value: (L, OP, R)) -> Result<Self, Self::Error> {
        Ok(SearchType::Tuple(value.try_into()?))
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ErrorType {
    #[error(transparent)]
    InvalidDomain(#[from] InvalidDomainError),
    #[error(transparent)]
    UnknownSearchOperator(#[from] UnknownSearchOperatorError),
    #[error("The domain nests operators of different kinds more than {0} deep")]
    TooDeep(usize),
}

/// How deeply operators of different kinds may nest in a domain. Long runs of one operator do
/// not count: they are folded into a balanced tree, so a list of thousands of conditions is fine.
pub const MAX_DOMAIN_NESTING: usize = 64;

/// A domain while it is read: runs of one operator gathered into one list.
enum Node {
    Tuple(SearchTuple),
    Operator(SearchKey, VecDeque<Node>, usize),
}

impl Node {
    fn nesting(&self) -> usize {
        match self {
            Node::Tuple(_) => 0,
            Node::Operator(_, _, nesting) => *nesting,
        }
    }

    /// `operator` over two operands, gathering them into its run when they are runs of it too.
    /// Growing a run at either end costs nothing, whatever its length.
    fn combine(operator: SearchKey, left: Node, right: Node) -> Node {
        match (left, right) {
            (Node::Operator(key, mut run, mut nesting), other) if key == operator => {
                Node::append(&operator, &mut run, &mut nesting, other, false);
                Node::Operator(operator, run, nesting)
            }
            (other, Node::Operator(key, mut run, mut nesting)) if key == operator => {
                Node::append(&operator, &mut run, &mut nesting, other, true);
                Node::Operator(operator, run, nesting)
            }
            (left, right) => {
                let nesting = left.nesting().max(right.nesting()) + 1;
                Node::Operator(operator, VecDeque::from([left, right]), nesting)
            }
        }
    }

    /// Put an operand at one end of a run of `operator`, merging it in if it is a run of it too.
    fn append(
        operator: &SearchKey,
        run: &mut VecDeque<Node>,
        nesting: &mut usize,
        operand: Node,
        at_front: bool,
    ) {
        match operand {
            Node::Operator(key, inner, inner_nesting) if key == *operator => {
                *nesting = (*nesting).max(inner_nesting);
                if at_front {
                    for node in inner.into_iter().rev() {
                        run.push_front(node);
                    }
                } else {
                    run.extend(inner);
                }
            }
            other => {
                *nesting = (*nesting).max(other.nesting() + 1);
                if at_front {
                    run.push_front(other);
                } else {
                    run.push_back(other);
                }
            }
        }
    }

    /// The tree a run becomes: balanced, so its depth grows with the log of its length.
    fn into_search_type(self) -> SearchType {
        match self {
            Node::Tuple(tuple) => SearchType::Tuple(tuple),
            Node::Operator(key, operands, _) => {
                let mut level: Vec<SearchType> =
                    operands.into_iter().map(Node::into_search_type).collect();
                while level.len() > 1 {
                    let mut next = Vec::with_capacity(level.len().div_ceil(2));
                    let mut pairs = level.into_iter();
                    while let Some(left) = pairs.next() {
                        next.push(match pairs.next() {
                            Some(right) if key == SearchKey::Or => {
                                SearchType::Or(Box::new(left), Box::new(right))
                            }
                            Some(right) => SearchType::And(Box::new(left), Box::new(right)),
                            None => left,
                        });
                    }
                    level = next;
                }
                level.pop().unwrap_or(SearchType::Nothing)
            }
        }
    }
}

impl TryFrom<Vec<SearchKey>> for SearchType {
    type Error = ErrorType;

    /// Read a domain in prefix notation, without recursion: from its end, each condition is put
    /// on a stack, and each operator takes the two on top. What is left is ANDed. Refused when
    /// an operator lacks operands, or operators of different kinds nest too deeply.
    fn try_from(value: Vec<SearchKey>) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Ok(SearchType::Nothing);
        }
        let invalid = |value: &Vec<SearchKey>| {
            ErrorType::InvalidDomain(InvalidDomainError {
                search_key: value.clone(),
            })
        };
        let mut stack: Vec<Node> = Vec::new();
        for key in value.iter().rev() {
            let node = match key {
                SearchKey::Tuple(tuple) => Node::Tuple(tuple.clone()),
                operator => {
                    let (Some(left), Some(right)) = (stack.pop(), stack.pop()) else {
                        return Err(invalid(&value));
                    };
                    Node::combine(operator.clone(), left, right)
                }
            };
            if node.nesting() > MAX_DOMAIN_NESTING {
                return Err(ErrorType::TooDeep(MAX_DOMAIN_NESTING));
            }
            stack.push(node);
        }
        let mut expressions = stack.into_iter().rev();
        let Some(first) = expressions.next() else {
            return Err(invalid(&value));
        };
        let all = expressions.fold(first, |left, right| {
            Node::combine(SearchKey::And, left, right)
        });
        if all.nesting() > MAX_DOMAIN_NESTING {
            return Err(ErrorType::TooDeep(MAX_DOMAIN_NESTING));
        }
        Ok(all.into_search_type())
    }
}

impl<E> TryFrom<Vec<E>> for SearchType
where
    E: TryInto<SearchKey, Error = UnknownSearchOperatorError>,
{
    type Error = ErrorType;

    fn try_from(value: Vec<E>) -> Result<Self, Self::Error> {
        let result_values: Vec<Result<SearchKey, _>> =
            value.into_iter().map(|val| val.try_into()).collect();
        for val in &result_values {
            if let Err(err) = val {
                return Err(ErrorType::UnknownSearchOperator(err.clone()));
            }
        }

        let value: Vec<SearchKey> = result_values.into_iter().map(|val| val.unwrap()).collect();

        value.try_into()
    }
}
