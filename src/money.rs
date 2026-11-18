//! Money is stored as whole hundredths (pence, cents) in an integer so that
//! totals never suffer from floating point rounding. This mirrors the
//! `DECIMAL(8,2)` column in `assets/schema/mariadb.sql`. Locale-aware
//! formatting and input cleaning live in `locale.rs`; this module only deals
//! with the canonical form `-1234.56`.

/// Largest value that fits in DECIMAL(8,2): 999999.99
pub const MAX_HUNDREDTHS: i64 = 99_999_999;

#[derive(Debug, PartialEq)]
pub enum ParseError {
    NotANumber,
    TooLarge,
}

/// Parse a canonical amount: optional leading `-`, digits, and an optional
/// `.` followed by one or two digits. No symbols or separators.
pub fn parse_canonical(input: &str) -> Result<i64, ParseError> {
    let (negative, digits) = match input.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, input),
    };
    let (whole, frac) = digits.split_once('.').unwrap_or((digits, ""));
    let all_digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    if (whole.is_empty() && frac.is_empty()) || !all_digits(whole) || !all_digits(frac) || frac.len() > 2 {
        return Err(ParseError::NotANumber);
    }
    let whole: i64 = if whole.is_empty() { 0 } else { whole.parse().map_err(|_| ParseError::TooLarge)? };
    let frac: i64 = match frac.len() {
        0 => 0,
        1 => frac.parse::<i64>().unwrap() * 10,
        _ => frac.parse().unwrap(),
    };
    let value = whole
        .checked_mul(100)
        .and_then(|p| p.checked_add(frac))
        .filter(|p| *p <= MAX_HUNDREDTHS)
        .ok_or(ParseError::TooLarge)?;
    Ok(if negative { -value } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_canonical_forms() {
        assert_eq!(parse_canonical("12.50"), Ok(1250));
        assert_eq!(parse_canonical("12.5"), Ok(1250));
        assert_eq!(parse_canonical("12"), Ok(1200));
        assert_eq!(parse_canonical(".99"), Ok(99));
        assert_eq!(parse_canonical("-3.99"), Ok(-399));
        assert_eq!(parse_canonical("999999.99"), Ok(MAX_HUNDREDTHS));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in ["", "abc", "1.234", "1.2.3", "-", ".", "12p", "1,000"] {
            assert_eq!(parse_canonical(bad), Err(ParseError::NotANumber), "{bad}");
        }
        assert_eq!(parse_canonical("1000000"), Err(ParseError::TooLarge));
    }
}
