//! Reading a domain without recursion: long runs stay shallow, deep nesting is refused.

use erp_search::{
    ErrorType, LeftTuple, RightTuple, SearchKey, SearchOperator, SearchTuple, SearchType,
};

fn tuple(value: i32) -> SearchKey {
    SearchKey::Tuple(SearchTuple {
        left: LeftTuple {
            path: vec!["amount".to_string()],
        },
        operator: SearchOperator::Equal,
        right: RightTuple::Integer(value),
    })
}

fn depth(domain: &SearchType) -> usize {
    match domain {
        SearchType::And(left, right) | SearchType::Or(left, right) => {
            1 + depth(left).max(depth(right))
        }
        _ => 0,
    }
}

fn conditions(domain: &SearchType) -> usize {
    match domain {
        SearchType::And(left, right) | SearchType::Or(left, right) => {
            conditions(left) + conditions(right)
        }
        SearchType::Tuple(_) => 1,
        _ => 0,
    }
}

/// A hundred thousand conditions, ANDed by being listed, make a balanced tree.
#[test]
fn test_a_long_list_stays_shallow() {
    let keys: Vec<SearchKey> = (0..100_000).map(tuple).collect();
    let domain = SearchType::try_from(keys).expect("a domain");
    assert_eq!(conditions(&domain), 100_000);
    assert!(depth(&domain) <= 17, "depth {}", depth(&domain));
}

/// A long run of one operator, as a client writes an OR of many values, stays shallow too.
#[test]
fn test_a_long_run_of_one_operator_stays_shallow() {
    let mut keys = vec![SearchKey::Or; 9_999];
    keys.extend((0..10_000).map(tuple));
    let domain = SearchType::try_from(keys).expect("a domain");
    assert_eq!(conditions(&domain), 10_000);
    assert!(depth(&domain) <= 14, "depth {}", depth(&domain));
    assert!(matches!(domain, SearchType::Or(_, _)));
}

/// Operators of different kinds nesting deeper than the limit are refused, not followed.
#[test]
fn test_deep_nesting_is_refused() {
    let mut keys = Vec::new();
    for level in 0..100_000 {
        keys.push(if level % 2 == 0 {
            SearchKey::And
        } else {
            SearchKey::Or
        });
        keys.push(tuple(level));
    }
    keys.push(tuple(-1));
    assert!(matches!(
        SearchType::try_from(keys),
        Err(ErrorType::TooDeep(_))
    ));
}

/// An operator short of operands is still refused.
#[test]
fn test_an_operator_short_of_operands_is_refused() {
    assert!(matches!(
        SearchType::try_from(vec![SearchKey::And, tuple(1)]),
        Err(ErrorType::InvalidDomain(_))
    ));
    assert!(matches!(
        SearchType::try_from(vec![tuple(1), SearchKey::Or]),
        Err(ErrorType::InvalidDomain(_))
    ));
}

/// Mixed operators keep their meaning: `| a & b c` is a OR (b AND c).
#[test]
fn test_mixed_operators_keep_their_meaning() {
    let domain = SearchType::try_from(vec![
        SearchKey::Or,
        tuple(1),
        SearchKey::And,
        tuple(2),
        tuple(3),
    ])
    .expect("a domain");
    let SearchType::Or(left, right) = domain else {
        panic!("an OR at the top")
    };
    assert!(matches!(*left, SearchType::Tuple(_)));
    assert!(matches!(*right, SearchType::And(_, _)));
}
