use erp::search::{SearchKey, SearchOperator, SearchTuple, SearchType};

#[test]
fn test_domain() -> Result<(), Box<dyn std::error::Error>> {
    let domain: SearchType = ("test", "=", "lol").into();
    assert_eq!(domain, SearchType::Tuple(SearchTuple {
        left: "test".to_string(),
        operator: SearchOperator::Equal,
        right: "lol".to_string(),
    }));

    let domain: SearchType = vec![("test", "=", "lol")].try_into()?;
    assert_eq!(domain, SearchType::Tuple(SearchTuple {
        left: "test".to_string(),
        operator: SearchOperator::Equal,
        right: "lol".to_string(),
    }));

    let domain: SearchType = vec![("test", "=", "lol"), ("test", "=", "lol2")].try_into()?;
    assert_eq!(domain, SearchType::And(Box::new(SearchType::Tuple(SearchTuple {
        left: "test".to_string(),
        operator: SearchOperator::Equal,
        right: "lol".to_string(),
    })), Box::new(SearchType::Tuple(SearchTuple {
        left: "test".to_string(),
        operator: SearchOperator::Equal,
        right: "lol2".to_string(),
    }))));

    // Here the "into()" is needed, as we are mixing string & tuple.
    // Also, we need to do this in 2 steps, we will try to find a way to have it in one step later
    let domain: Vec<SearchKey> = vec!["|".into(), ("test", "=", "lol").into(), ("test", "=", "lol2").into()];
    let domain: SearchType = domain.try_into()?;
    assert_eq!(domain, SearchType::Or(Box::new(SearchType::Tuple(SearchTuple {
        left: "test".to_string(),
        operator: SearchOperator::Equal,
        right: "lol".to_string(),
    })), Box::new(SearchType::Tuple(SearchTuple {
        left: "test".to_string(),
        operator: SearchOperator::Equal,
        right: "lol2".to_string(),
    }))));

    // Add an extra argument
    let domain: Vec<SearchKey> = vec!["|".into(), ("test", "=", "lol").into(), ("test", "=", "lol2").into(), ("test", "=", "lol3").into()];
    let domain: SearchType = domain.try_into()?;
    assert_eq!(domain, SearchType::And(
        Box::new(SearchType::Or(
            Box::new(SearchType::Tuple(SearchTuple {
                left: "test".to_string(),
                operator: SearchOperator::Equal,
                right: "lol".to_string(),
            })),
            Box::new(SearchType::Tuple(*Box::new(SearchTuple {
                left: "test".to_string(),
                operator: SearchOperator::Equal,
                right: "lol2".to_string(),
            })))
        )),
        Box::new(SearchType::Tuple(SearchTuple {
            left: "test".to_string(),
            operator: SearchOperator::Equal,
            right: "lol3".to_string(),
        }))));

    // TODO Add more tests

    Ok(())
}
