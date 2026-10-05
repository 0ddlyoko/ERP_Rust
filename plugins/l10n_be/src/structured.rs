//! Belgian structured communication, `+++123/4567/89002+++`: ten digits and two check digits,
//! the remainder of the ten by 97 (97 when it is nought).

/// The structured communication of `number` (its last ten digits).
pub fn structured_communication(number: u64) -> String {
    let base = number % 10_000_000_000;
    let check = match base % 97 {
        0 => 97,
        rest => rest,
    };
    let digits = format!("{base:010}{check:02}");
    format!("+++{}/{}/{}+++", &digits[..3], &digits[3..7], &digits[7..])
}

/// Whether `text` is a structured communication with the right check digits, written with or
/// without its `+++`, `***`, slashes and spaces.
pub fn is_structured_communication(text: &str) -> bool {
    let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    let rest: String = text
        .chars()
        .filter(|c| !c.is_ascii_digit() && !matches!(c, '+' | '*' | '/' | ' '))
        .collect();
    if digits.len() != 12 || !rest.is_empty() {
        return false;
    }
    let base: u64 = digits[..10].parse().unwrap_or(0);
    let check: u64 = digits[10..].parse().unwrap_or(0);
    let expected = match base % 97 {
        0 => 97,
        rest => rest,
    };
    check == expected
}

/// A Belgian VAT or enterprise number, `BE0477472701`, from any way it is written; errs when its
/// check digits are wrong: 97 less the first eight digits modulo 97.
pub fn normalize_belgian_vat(text: &str) -> Result<String, String> {
    let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    let digits = match digits.len() {
        9 => format!("0{digits}"),
        10 => digits,
        _ => {
            return Err(format!(
                "{text} is not a Belgian VAT number: it has ten digits"
            ));
        }
    };
    if !digits.starts_with('0') && !digits.starts_with('1') {
        return Err(format!(
            "{text} is not a Belgian VAT number: it starts with 0 or 1"
        ));
    }
    let base: u64 = digits[..8]
        .parse()
        .map_err(|_| format!("{text} is not a number"))?;
    let check: u64 = digits[8..]
        .parse()
        .map_err(|_| format!("{text} is not a number"))?;
    if 97 - base % 97 != check {
        return Err(format!(
            "{text} is not a Belgian VAT number: its check digits are wrong"
        ));
    }
    Ok(format!("BE{digits}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_structured_communications() {
        assert_eq!(structured_communication(1), "+++000/0000/00101+++");
        assert_eq!(structured_communication(1234567890), "+++123/4567/89002+++");
        assert_eq!(
            structured_communication(97),
            "+++000/0000/09797+++",
            "a nought remainder is 97"
        );
        assert!(is_structured_communication("+++123/4567/89002+++"));
        assert!(is_structured_communication("123456789002"));
        assert!(is_structured_communication("***123/4567/89002***"));
        assert!(!is_structured_communication("+++123/4567/89003+++"));
        assert!(!is_structured_communication("INV/2026/00001"));
    }

    #[test]
    fn test_belgian_vat_numbers() {
        assert_eq!(
            normalize_belgian_vat("BE 0477.472.701"),
            Ok("BE0477472701".to_string())
        );
        assert_eq!(
            normalize_belgian_vat("be0477472701"),
            Ok("BE0477472701".to_string())
        );
        assert_eq!(
            normalize_belgian_vat("477472701"),
            Ok("BE0477472701".to_string()),
            "the old nine digits"
        );
        assert!(normalize_belgian_vat("BE0477472702").is_err());
        assert!(normalize_belgian_vat("BE047747270").is_err());
        assert!(normalize_belgian_vat("BE2477472701").is_err());
    }
}
