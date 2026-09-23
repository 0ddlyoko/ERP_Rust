//! Reading values and domains off the wire.
//!
//! The only surface an untrusted caller reaches directly, so what it refuses matters as much as
//! what it accepts.

use erp_search::{RightTuple, SearchOperator, SearchType};
use erp_types::field::{Decimal, FieldKind, FieldKinds, FieldType, MapOfFieldsSeed, NaiveDate};
use erp_types::model::MapOfFields;
use serde::de::DeserializeSeed;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn read(kind: FieldKind, json: &str) -> Result<FieldType> {
    let mut de = serde_json::Deserializer::from_str(json);
    Ok(kind.deserialize(&mut de)?)
}

fn kinds() -> HashMap<String, FieldKind> {
    HashMap::from([
        ("name".to_string(), FieldKind::String),
        ("amount".to_string(), FieldKind::Integer),
        ("price".to_string(), FieldKind::Decimal),
        ("active".to_string(), FieldKind::Bool),
        ("due_date".to_string(), FieldKind::Date),
        ("order".to_string(), FieldKind::Ref),
        ("tags".to_string(), FieldKind::Refs),
    ])
}

fn read_map(json: &str) -> Result<MapOfFields> {
    let kinds = kinds();
    let mut de = serde_json::Deserializer::from_str(json);
    Ok(MapOfFieldsSeed(&kinds as &dyn FieldKinds).deserialize(&mut de)?)
}

fn domain(json: &str) -> Result<SearchType> {
    Ok(serde_json::from_str(json)?)
}

/// A password arrives in clear and is hashed before it is a value at all.
///
/// The edge is the last place the clear password exists: nothing further in can be handed one by
/// mistake, because nothing further in is ever given one.
#[test]
fn test_a_password_is_hashed_as_it_is_read() -> Result<()> {
    let FieldType::Password(password) = read(FieldKind::Password, r#""hunter2""#)? else {
        panic!("a password");
    };

    assert!(password.is_same_password("hunter2"));
    assert!(!password.is_same_password("hunter3"));
    Ok(())
}

/// The declared kind decides, not the shape of the value.
#[test]
fn test_a_value_is_read_as_its_declared_kind() -> Result<()> {
    assert_eq!(
        read(FieldKind::String, r#""0ddlyoko""#)?,
        FieldType::String("0ddlyoko".to_string())
    );
    assert_eq!(read(FieldKind::Integer, "7")?, FieldType::Integer(7));
    assert_eq!(read(FieldKind::Bool, "true")?, FieldType::Bool(true));
    assert_eq!(read(FieldKind::Ref, "4")?, FieldType::Ref(4));
    assert_eq!(
        read(FieldKind::Refs, "[1, 2, 3]")?,
        FieldType::Refs(vec![1, 2, 3])
    );
    assert_eq!(
        read(FieldKind::Date, r#""2026-09-30""#)?,
        FieldType::Date(NaiveDate::from_str("2026-09-30")?)
    );
    Ok(())
}

/// `4` is an integer or the id of a record depending only on what the model says.
#[test]
fn test_the_same_json_reads_differently_by_kind() -> Result<()> {
    assert_eq!(read(FieldKind::Integer, "4")?, FieldType::Integer(4));
    assert_eq!(read(FieldKind::Ref, "4")?, FieldType::Ref(4));
    Ok(())
}

/// A decimal survives the trip exactly, which is the whole point of the type.
#[test]
fn test_a_decimal_is_read_exactly() -> Result<()> {
    assert_eq!(
        read(FieldKind::Decimal, r#""1234.56""#)?,
        FieldType::Decimal(Decimal::from_str("1234.56")?)
    );
    assert_eq!(
        read(FieldKind::Decimal, "1234")?,
        FieldType::Decimal(Decimal::from_str("1234")?),
        "a whole number is exact, so it is accepted"
    );
    Ok(())
}

/// A fraction arrives as a float, which would round it. Refused rather than quietly accepted.
#[test]
fn test_a_decimal_written_as_a_fraction_is_refused() {
    let err = read(FieldKind::Decimal, "0.1").unwrap_err().to_string();
    assert!(
        err.contains("string"),
        "the error must say how to send it, got: {err}"
    );
}

/// A value of the wrong shape is refused, not coerced.
#[test]
fn test_a_mistyped_value_is_refused() {
    assert!(read(FieldKind::Integer, r#""seven""#).is_err());
    assert!(read(FieldKind::Bool, "1").is_err());
    assert!(read(FieldKind::Refs, "4").is_err());
    assert!(read(FieldKind::Date, r#""30/09/2026""#).is_err());
}

/// A record reads against the kinds its model declares.
#[test]
fn test_a_record_is_read_field_by_field() -> Result<()> {
    let map = read_map(r#"{"name": "Order", "amount": 3, "price": "9.99", "tags": [1, 2]}"#)?;
    assert_eq!(map.get::<&String>("name"), &"Order".to_string());
    assert_eq!(map.get::<&i32>("amount"), &3);
    assert_eq!(map.get::<&Decimal>("price"), &Decimal::from_str("9.99")?);
    Ok(())
}

/// `null` clears a field, which is not the same as leaving it out.
#[test]
fn test_null_is_an_empty_value_and_absence_is_silence() -> Result<()> {
    let map = read_map(r#"{"name": null}"#)?;
    assert!(
        map.fields.contains_key("name"),
        "the field was named, so it is present"
    );
    assert!(
        map.get_option::<&String>("name").is_none(),
        "and it is empty"
    );

    let map = read_map(r#"{}"#)?;
    assert!(
        !map.fields.contains_key("name"),
        "a field nobody named must not be touched"
    );
    Ok(())
}

/// A field the model does not declare is refused, so a typo cannot pass for a write.
#[test]
fn test_an_undeclared_field_is_refused() {
    let err = read_map(r#"{"nmae": "typo"}"#).unwrap_err().to_string();
    assert!(
        err.contains("nmae"),
        "the error must name the offending field, got: {err}"
    );
}

/// A domain in prefix notation, the form the macro already speaks.
#[test]
fn test_a_domain_is_read_in_prefix_notation() -> Result<()> {
    let parsed = domain(r#"[["name", "=", "0ddlyoko"]]"#)?;
    let SearchType::Tuple(tuple) = parsed else {
        panic!("expected a single condition, got {parsed:?}");
    };
    assert_eq!(tuple.left.path, vec!["name".to_string()]);
    assert_eq!(tuple.operator, SearchOperator::Equal);
    assert_eq!(tuple.right, RightTuple::String("0ddlyoko".to_string()));
    Ok(())
}

/// `&` and `|` fold the conditions that follow them.
#[test]
fn test_and_and_or_are_read() -> Result<()> {
    assert!(matches!(
        domain(r#"["&", ["a", "=", 1], ["b", "=", 2]]"#)?,
        SearchType::And(..)
    ));
    assert!(matches!(
        domain(r#"["|", ["a", "=", 1], ["b", "=", 2]]"#)?,
        SearchType::Or(..)
    ));
    Ok(())
}

/// A path crosses models exactly as it does in the macro.
#[test]
fn test_a_domain_crosses_relations() -> Result<()> {
    let parsed = domain(r#"[["order.tags.name", "=", "urgent"]]"#)?;
    let SearchType::Tuple(tuple) = parsed else {
        panic!("expected a single condition");
    };
    assert_eq!(tuple.left.path, vec!["order", "tags", "name"]);
    Ok(())
}

/// An empty domain selects everything, and says so in the type.
#[test]
fn test_an_empty_domain_is_nothing() -> Result<()> {
    assert_eq!(domain("[]")?, SearchType::Nothing);
    Ok(())
}

/// Lists on the right-hand side, for `in`.
#[test]
fn test_a_domain_reads_a_list() -> Result<()> {
    let parsed = domain(r#"[["name", "in", ["a", "b"]]]"#)?;
    let SearchType::Tuple(tuple) = parsed else {
        panic!("expected a single condition");
    };
    assert_eq!(tuple.operator, SearchOperator::In);
    assert_eq!(
        tuple.right,
        RightTuple::Array(vec![
            RightTuple::String("a".to_string()),
            RightTuple::String("b".to_string()),
        ])
    );
    Ok(())
}

/// Malformed domains are refused, each for its own reason.
#[test]
fn test_malformed_domains_are_refused() {
    assert!(
        domain(r#"[["name", "=~=", "x"]]"#).is_err(),
        "unknown operator"
    );
    assert!(domain(r#"[["name", "="]]"#).is_err(), "missing value");
    assert!(
        domain(r#"[["name", "=", "x", "extra"]]"#).is_err(),
        "a condition is exactly three elements"
    );
    assert!(
        domain(r#"["&", ["a", "=", 1]]"#).is_err(),
        "and needs two sides"
    );
    assert!(domain(r#"["nope"]"#).is_err(), "unknown key");
    assert!(
        domain(r#"[[1, "=", 2]]"#).is_err(),
        "the left side is a path"
    );
}

/// What goes out comes back identical.
///
/// The two sides are written separately — a custom `Serialize` and a kind-directed seed — so
/// nothing but a test keeps them agreeing.
#[test]
fn test_a_record_survives_the_round_trip() -> Result<()> {
    let mut sent = MapOfFields::default();
    sent.insert("name", "0ddlyoko");
    sent.insert("amount", 7);
    sent.insert("price", Decimal::from_str("1234.56")?);
    sent.insert("active", true);
    sent.insert("due_date", NaiveDate::from_str("2026-09-30")?);
    sent.insert("order", FieldType::Ref(4));
    sent.insert("tags", FieldType::Refs(vec![1, 2, 3]));
    sent.insert_none("name");
    sent.insert("name", "0ddlyoko");

    let wire = serde_json::to_string(&sent)?;
    let back = read_map(&wire)?;

    let mut names: Vec<&String> = sent.fields.keys().collect();
    names.sort();
    let mut back_names: Vec<&String> = back.fields.keys().collect();
    back_names.sort();
    assert_eq!(names, back_names, "every field must come back");

    for name in names {
        assert_eq!(
            sent.fields.get(name),
            back.fields.get(name),
            "field {name} changed on the way back, wire form was {wire}"
        );
    }
    Ok(())
}

/// An empty value survives it too.
#[test]
fn test_an_empty_value_survives_the_round_trip() -> Result<()> {
    let mut sent = MapOfFields::default();
    sent.insert_none("name");

    let back = read_map(&serde_json::to_string(&sent)?)?;
    assert!(back.fields.contains_key("name"));
    assert!(back.get_option::<&String>("name").is_none());
    Ok(())
}

/// A whole number from the wire matches the column that holds it, whatever its sign.
///
/// JSON has one kind of number and no sign information, so a positive integer arrives unsigned
/// while the column may be signed — and an id is unsigned while a count is not. Compared by
/// shape rather than by value, an ordinary domain matched nothing, in silence.
#[test]
fn test_a_whole_number_matches_a_signed_or_unsigned_column() -> Result<()> {
    let signed = domain(r#"[["amount", "=", 10]]"#)?;
    let SearchType::Tuple(tuple) = signed else {
        panic!("expected one condition");
    };
    assert_eq!(
        tuple.right,
        RightTuple::UInteger(10),
        "serde hands a positive whole number over unsigned"
    );

    // What matters is that it compares equal to either shape.
    use erp::database::FieldType as Stored;
    assert!(
        Stored::Integer(10) == tuple.right,
        "against a signed column"
    );
    assert!(
        Stored::UInteger(10) == tuple.right,
        "against an unsigned one"
    );
    assert!(Stored::Integer(11) != tuple.right);

    let negative = domain(r#"[["amount", "=", -3]]"#)?;
    let SearchType::Tuple(tuple) = negative else {
        panic!("expected one condition");
    };
    assert_eq!(tuple.right, RightTuple::Integer(-3));
    assert!(Stored::Integer(-3) == tuple.right);
    Ok(())
}
