use erp_search::{RightTuple, SearchOperator, SearchTuple, SearchType};
use erp_search_code_gen::make_domain;

#[test]
fn test_domain_macro() -> Result<(), Box<dyn std::error::Error>> {
    // Empty
    let domain = make_domain!([]);
    assert_eq!(domain, SearchType::Nothing);

    Ok(())
}

#[test]
fn test_domain_macro_copy() -> Result<(), Box<dyn std::error::Error>> {
    // Simple one
    let domain = make_domain!([("test", "=", "lol")]);
    assert_eq!(domain, SearchType::Tuple(SearchTuple {
        left: "test".into(),
        operator: SearchOperator::Equal,
        right: RightTuple::String("lol".to_string()),
    }));

    let domain = make_domain!([("test", "=", "lol"), ("test", "=", "lol2")]);
    assert_eq!(domain, SearchType::And(
        Box::new(SearchType::Tuple(SearchTuple {
            left: "test".into(),
            operator: SearchOperator::Equal,
            right: RightTuple::String("lol".to_string()),
        })),
        Box::new(SearchType::Tuple(SearchTuple {
            left: "test".into(),
            operator: SearchOperator::Equal,
            right: RightTuple::String("lol2".to_string()),
        })),
    ));

    let domain = make_domain!(["|", ("test", "=", "lol"), ("test", "=", "lol2")]);
    assert_eq!(domain, SearchType::Or(
        Box::new(SearchType::Tuple(SearchTuple {
            left: "test".into(),
            operator: SearchOperator::Equal,
            right: RightTuple::String("lol".to_string()),
        })), Box::new(SearchType::Tuple(SearchTuple {
            left: "test".into(),
            operator: SearchOperator::Equal,
            right: RightTuple::String("lol2".to_string()),
        }))
    ));

    // Add an extra argument
    let domain = make_domain!(["|", ("test", "=", "lol"), ("test", "=", "lol2"), ("test", "=", "lol3")]);
    assert_eq!(domain, SearchType::And(
        Box::new(SearchType::Or(
            Box::new(SearchType::Tuple(SearchTuple {
                left: "test".into(),
                operator: SearchOperator::Equal,
                right: RightTuple::String("lol".to_string()),
            })),
            Box::new(SearchType::Tuple(SearchTuple {
                left: "test".into(),
                operator: SearchOperator::Equal,
                right: RightTuple::String("lol2".to_string()),
            }))
        )),
        Box::new(SearchType::Tuple(SearchTuple {
            left: "test".into(),
            operator: SearchOperator::Equal,
            right: RightTuple::String("lol3".to_string()),
        }))
    ));

    // None
    let domain = make_domain!([("test", "=", None)]);
    assert_eq!(domain, SearchType::Tuple(SearchTuple {
        left: "test".into(),
        operator: SearchOperator::Equal,
        right: RightTuple::None,
    }));

    // Array
    let domain = make_domain!([("test", "=", Vec::<u32>::new())]);
    assert_eq!(domain, SearchType::Tuple(SearchTuple {
        left: "test".into(),
        operator: SearchOperator::Equal,
        right: RightTuple::Array(vec![]),
    }));
    let domain = make_domain!([("test", "=", vec![1, 2, 3, 4])]);
    assert_eq!(domain, SearchType::Tuple(SearchTuple {
        left: "test".into(),
        operator: SearchOperator::Equal,
        right: RightTuple::Array(vec![
            RightTuple::Integer(1),
            RightTuple::Integer(2),
            RightTuple::Integer(3),
            RightTuple::Integer(4),
        ]),
    }));

    // TODO Add more tests

    Ok(())
}
