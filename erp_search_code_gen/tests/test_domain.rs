use erp_search::{SearchOperator, SearchTuple, SearchType};
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
        left: "test".to_string(),
        operator: SearchOperator::Equal,
        right: "lol".to_string(),
    }));

    let domain = make_domain!([("test", "=", "lol"), ("test", "=", "lol2")]);
    assert_eq!(domain, SearchType::And(
        Box::new(SearchType::Tuple(SearchTuple {
            left: "test".to_string(),
            operator: SearchOperator::Equal,
            right: "lol".to_string(),
        })),
        Box::new(SearchType::Tuple(SearchTuple {
            left: "test".to_string(),
            operator: SearchOperator::Equal,
            right: "lol2".to_string(),
        })),
    ));

    let domain = make_domain!(["|", ("test", "=", "lol"), ("test", "=", "lol2")]);
    assert_eq!(domain, SearchType::Or(
        Box::new(SearchType::Tuple(SearchTuple {
            left: "test".to_string(),
            operator: SearchOperator::Equal,
            right: "lol".to_string(),
        })), Box::new(SearchType::Tuple(SearchTuple {
            left: "test".to_string(),
            operator: SearchOperator::Equal,
            right: "lol2".to_string(),
        }))
    ));

    // Add an extra argument
    let domain = make_domain!(["|", ("test", "=", "lol"), ("test", "=", "lol2"), ("test", "=", "lol3")]);
    assert_eq!(domain, SearchType::And(
        Box::new(SearchType::Or(
            Box::new(SearchType::Tuple(SearchTuple {
                left: "test".to_string(),
                operator: SearchOperator::Equal,
                right: "lol".to_string(),
            })),
            Box::new(SearchType::Tuple(SearchTuple {
                left: "test".to_string(),
                operator: SearchOperator::Equal,
                right: "lol2".to_string(),
            }))
        )),
        Box::new(SearchType::Tuple(SearchTuple {
            left: "test".to_string(),
            operator: SearchOperator::Equal,
            right: "lol3".to_string(),
        }))
    ));

    // TODO Add more tests

    Ok(())
}
