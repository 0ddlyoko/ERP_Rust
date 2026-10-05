//! Turning a number into a document name, apart from the records.

use chrono::Datelike;
use erp::types::field::NaiveDate;

/// `prefix`, `number` padded with zeros to `padding` digits, then `suffix`, with `{year}`,
/// `{y}` (two digits), `{month}` and `{day}` replaced by those of `date`.
///
/// `format_name("INV/{year}/", 42, 5, "", 2026-03-01)` is `INV/2026/00042`.
pub fn format_name(
    prefix: &str,
    number: i32,
    padding: i32,
    suffix: &str,
    date: NaiveDate,
) -> String {
    let padding = padding.clamp(0, 20) as usize;
    format!(
        "{}{number:0padding$}{}",
        interpolate(prefix, date),
        interpolate(suffix, date)
    )
}

/// The placeholders of `text` replaced by the parts of `date`.
pub fn interpolate(text: &str, date: NaiveDate) -> String {
    text.replace("{year}", &format!("{:04}", date.year()))
        .replace("{y}", &format!("{:02}", date.year() % 100))
        .replace("{month}", &format!("{:02}", date.month()))
        .replace("{day}", &format!("{:02}", date.day()))
}

/// The period a number belongs to when the series restarts: the year, the month, or none.
pub fn period(date: NaiveDate, yearly: bool, monthly: bool) -> String {
    if monthly {
        format!("{:04}-{:02}", date.year(), date.month())
    } else if yearly {
        format!("{:04}", date.year())
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date")
    }

    #[test]
    fn test_names_are_padded_and_dated() {
        assert_eq!(
            format_name("INV/{year}/", 42, 5, "", date("2026-03-01")),
            "INV/2026/00042"
        );
        assert_eq!(format_name("SO", 7, 3, "", date("2026-03-01")), "SO007");
        assert_eq!(
            format_name("{y}{month}-", 123456, 4, "-{day}", date("2026-03-09")),
            "2603-123456-09",
            "a number longer than the padding is kept whole"
        );
        assert_eq!(format_name("", 1, 0, "", date("2026-03-01")), "1");
    }

    #[test]
    fn test_periods() {
        assert_eq!(period(date("2026-03-09"), true, false), "2026");
        assert_eq!(period(date("2026-03-09"), false, true), "2026-03");
        assert_eq!(period(date("2026-03-09"), false, false), "");
    }
}
