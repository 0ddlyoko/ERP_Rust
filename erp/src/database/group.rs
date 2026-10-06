use crate::database::FieldType;
use chrono::{Datelike, Months, NaiveDate, TimeDelta};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::fmt;

/// How long a period records of a date are gathered by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Day,
    /// From Monday.
    Week,
    Month,
    Quarter,
    Year,
}

impl Period {
    pub const KEYS: [&str; 5] = ["day", "week", "month", "quarter", "year"];

    pub fn from_key(key: &str) -> Option<Period> {
        match key {
            "day" => Some(Period::Day),
            "week" => Some(Period::Week),
            "month" => Some(Period::Month),
            "quarter" => Some(Period::Quarter),
            "year" => Some(Period::Year),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Period::Day => "day",
            Period::Week => "week",
            Period::Month => "month",
            Period::Quarter => "quarter",
            Period::Year => "year",
        }
    }

    /// The first day of the period `date` falls in.
    pub fn start(self, date: NaiveDate) -> NaiveDate {
        match self {
            Period::Day => date,
            Period::Week => {
                date - TimeDelta::days(i64::from(date.weekday().num_days_from_monday()))
            }
            Period::Month => date.with_day(1).unwrap_or(date),
            Period::Quarter => {
                let month = (date.month0() / 3) * 3 + 1;
                NaiveDate::from_ymd_opt(date.year(), month, 1).unwrap_or(date)
            }
            Period::Year => NaiveDate::from_ymd_opt(date.year(), 1, 1).unwrap_or(date),
        }
    }

    /// The first day of the period after the one starting on `start`.
    pub fn next(self, start: NaiveDate) -> NaiveDate {
        match self {
            Period::Day => start + TimeDelta::days(1),
            Period::Week => start + TimeDelta::days(7),
            Period::Month => start + Months::new(1),
            Period::Quarter => start + Months::new(3),
            Period::Year => start + Months::new(12),
        }
    }
}

impl fmt::Display for Period {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.key())
    }
}

/// What records are gathered by: a field's value, or for a date, the period it falls in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupBy {
    pub field: String,
    pub period: Option<Period>,
}

impl GroupBy {
    /// `state`, or `date_order:month` for a date by period.
    pub fn parse(text: &str) -> Result<GroupBy, String> {
        match text.split_once(':') {
            None => Ok(GroupBy {
                field: text.to_string(),
                period: None,
            }),
            Some((field, period)) => Ok(GroupBy {
                field: field.to_string(),
                period: Some(Period::from_key(period).ok_or_else(|| {
                    format!(
                        "\"{period}\" is not a period to group by: {}",
                        Period::KEYS.join(", ")
                    )
                })?),
            }),
        }
    }
}

/// Records sharing a value: the value — the first day of its period for a date gathered by
/// one — how many they are, and what their numbers add up to. `None` gathers those with none.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub key: Option<FieldType>,
    pub count: u32,
    pub sums: HashMap<String, Decimal>,
}
