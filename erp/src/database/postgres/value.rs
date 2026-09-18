use crate::database::FieldType;
use erp_types::field::FieldKind;
use postgres::Row;
use postgres::types::ToSql;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Ids are `u32` in the framework and `INTEGER` in PostgreSQL, which is signed. The cast is
/// checked rather than silent: an id past this point would come back as a negative number.
pub(crate) fn id_to_sql(id: u32) -> Result<i32> {
    i32::try_from(id)
        .map_err(|_| format!("Record id {id} does not fit in a PostgreSQL INTEGER").into())
}

/// The reverse, guarding against a column that somehow holds a negative id.
pub(crate) fn id_from_sql(id: i32) -> Result<u32> {
    u32::try_from(id)
        .map_err(|_| format!("Read a negative record id ({id}) from the database").into())
}

/// Borrow a value as something the driver can bind.
///
/// `u32` has no `ToSql`, so references travel as `i32`; everything else binds natively.
pub(crate) fn to_sql_param(value: &FieldType) -> Result<Box<dyn ToSql + Sync + Send>> {
    Ok(match value {
        FieldType::String(value) => Box::new(value.clone()),
        FieldType::Integer(value) => Box::new(*value),
        FieldType::UInteger(value) => Box::new(id_to_sql(*value)?),
        FieldType::Decimal(value) => Box::new(*value),
        FieldType::Boolean(value) => Box::new(*value),
        FieldType::Date(value) => Box::new(*value),
        FieldType::DateTime(value) => Box::new(*value),
    })
}

/// Read a column back into the value of the kind the registry declares for it.
///
/// The kind drives the conversion because SQL alone cannot distinguish an integer column that
/// holds a number from one that holds a foreign key.
pub(crate) fn from_row(row: &Row, index: usize, kind: FieldKind) -> Result<Option<FieldType>> {
    Ok(match kind {
        FieldKind::String => row
            .try_get::<_, Option<String>>(index)?
            .map(FieldType::String),
        FieldKind::Integer => row
            .try_get::<_, Option<i32>>(index)?
            .map(FieldType::Integer),
        FieldKind::Decimal => row
            .try_get::<_, Option<rust_decimal::Decimal>>(index)?
            .map(FieldType::Decimal),
        FieldKind::Bool => row
            .try_get::<_, Option<bool>>(index)?
            .map(FieldType::Boolean),
        FieldKind::Date => row
            .try_get::<_, Option<chrono::NaiveDate>>(index)?
            .map(FieldType::Date),
        FieldKind::DateTime => row
            .try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(index)?
            .map(FieldType::DateTime),
        FieldKind::Ref => match row.try_get::<_, Option<i32>>(index)? {
            Some(id) => Some(FieldType::UInteger(id_from_sql(id)?)),
            None => None,
        },
        // A one2many has no column of its own; it is read from the other side's foreign key.
        FieldKind::Refs => None,
    })
}

/// PostgreSQL column type backing a field kind.
pub(crate) fn column_type(kind: FieldKind) -> Option<&'static str> {
    Some(match kind {
        FieldKind::String => "TEXT",
        FieldKind::Integer => "INTEGER",
        FieldKind::Decimal => "NUMERIC",
        FieldKind::Bool => "BOOLEAN",
        FieldKind::Date => "DATE",
        FieldKind::DateTime => "TIMESTAMPTZ",
        FieldKind::Ref => "INTEGER",
        FieldKind::Refs => return None,
    })
}

/// Quote an identifier so a field named `order` or `user` cannot be read as a keyword.
pub(crate) fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
