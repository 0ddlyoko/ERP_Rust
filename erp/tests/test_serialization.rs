use erp_types::field::{DateTime, Decimal, FieldType, NaiveDate, Utc};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn json(value: &FieldType) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

/// Values go out bare: the type lives in the field's metadata, not in each value.
#[test]
fn test_values_serialize_without_a_tag() -> Result<()> {
    assert_eq!(json(&FieldType::String("hello".into()))?, "\"hello\"");
    assert_eq!(json(&FieldType::Integer(-42))?, "-42");
    assert_eq!(json(&FieldType::Bool(true))?, "true");
    assert_eq!(json(&FieldType::Ref(7))?, "7");
    assert_eq!(json(&FieldType::Refs(vec![1, 2, 3]))?, "[1,2,3]");
    Ok(())
}

/// A decimal must never become a JSON number, or it would travel through a float.
#[test]
fn test_decimal_serializes_as_a_string() -> Result<()> {
    let value = FieldType::Decimal(Decimal::from_str("99999.99")?);
    assert_eq!(json(&value)?, "\"99999.99\"");

    let tricky = FieldType::Decimal(Decimal::from_str("0.1")?);
    assert_eq!(json(&tricky)?, "\"0.1\"");
    Ok(())
}

#[test]
fn test_dates_serialize_as_iso8601() -> Result<()> {
    let date = FieldType::Date(NaiveDate::from_str("2026-03-15")?);
    assert_eq!(json(&date)?, "\"2026-03-15\"");

    let stamp = FieldType::DateTime(DateTime::<Utc>::from_str("2026-01-02T10:30:00Z")?);
    assert_eq!(json(&stamp)?, "\"2026-01-02T10:30:00+00:00\"");
    Ok(())
}

#[test]
fn test_map_of_fields_serializes_as_an_object() -> Result<()> {
    let mut map = MapOfFields::new(HashMap::new());
    map.insert("name", "Bob");
    map.insert_none("email");

    let value: serde_json::Value = serde_json::from_str(&serde_json::to_string(&map)?)?;
    assert_eq!(value["name"], serde_json::json!("Bob"));
    assert_eq!(value["email"], serde_json::Value::Null);
    assert!(
        value.get("fields").is_none(),
        "the wrapper struct must not leak into the payload"
    );
    Ok(())
}

/// Parsing is driven by a template, because a bare value cannot say what it is.
#[test]
fn test_parse_is_driven_by_the_template() -> Result<()> {
    let as_integer = FieldType::Integer(0).parse_like("4")?;
    assert_eq!(as_integer, FieldType::Integer(4));

    let as_reference = FieldType::Ref(0).parse_like("4")?;
    assert_eq!(as_reference, FieldType::Ref(4));

    assert_ne!(
        as_integer.type_name(),
        as_reference.type_name(),
        "the same text yields different types depending on the field"
    );
    Ok(())
}

/// Every variant survives a write/read cycle through its textual form.
#[test]
fn test_every_variant_round_trips() -> Result<()> {
    let cases = vec![
        FieldType::String("hello".into()),
        FieldType::Integer(-42),
        FieldType::Decimal(Decimal::from_str("99999.99")?),
        FieldType::Bool(true),
        FieldType::Bool(false),
        FieldType::Date(NaiveDate::from_str("2026-03-15")?),
        FieldType::DateTime(DateTime::<Utc>::from_str("2026-01-02T10:30:00Z")?),
        FieldType::Ref(7),
        FieldType::Refs(vec![1, 2, 3]),
    ];

    for original in cases {
        let text = match &original {
            FieldType::Refs(ids) => ids.iter().map(u32::to_string).collect::<Vec<_>>().join(","),
            other => other.to_string(),
        };
        let parsed = original.parse_like(&text)?;
        assert_eq!(parsed, original, "round trip failed for {text:?}");
    }
    Ok(())
}

/// A decimal must come back exact, not merely close.
#[test]
fn test_decimal_round_trips_exactly() -> Result<()> {
    let original = Decimal::from_str("99999.99")?;
    let parsed = FieldType::Decimal(Decimal::ZERO).parse_like(&original.to_string())?;
    assert_eq!(parsed, FieldType::Decimal(original));

    let FieldType::Decimal(value) = parsed else {
        panic!("expected a decimal");
    };
    assert_eq!(
        value + Decimal::from_str("0.01")?,
        Decimal::from_str("100000.00")?
    );
    Ok(())
}

#[test]
fn test_booleans_accept_the_usual_spellings() -> Result<()> {
    for raw in ["true", "True", "1"] {
        assert_eq!(
            FieldType::Bool(false).parse_like(raw)?,
            FieldType::Bool(true)
        );
    }
    for raw in ["false", "False", "0"] {
        assert_eq!(
            FieldType::Bool(true).parse_like(raw)?,
            FieldType::Bool(false)
        );
    }
    Ok(())
}

/// Bad input is reported, and the message says what was expected.
#[test]
fn test_unparseable_text_is_reported() {
    let err = FieldType::Integer(0)
        .parse_like("not a number")
        .unwrap_err();
    assert!(err.to_string().contains("integer"), "got: {err}");

    let err = FieldType::Date(NaiveDate::default())
        .parse_like("32/13/2026")
        .unwrap_err();
    assert!(err.to_string().contains("date"), "got: {err}");

    let err = FieldType::Decimal(Decimal::ZERO)
        .parse_like("1.2.3")
        .unwrap_err();
    assert!(err.to_string().contains("decimal"), "got: {err}");
}
