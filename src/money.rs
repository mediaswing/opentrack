//! Money is held as integer pence (hundredths) to match `DECIMAL(8,2)` exactly.

/// Largest absolute value a `DECIMAL(8,2)` column can store: 999999.99
pub const MAX_PENCE: i64 = 99_999_999;

/// Longest currency text accepted around an amount, e.g. `CHF` or `лв.`.
const MAX_CURRENCY_CHARS: usize = 4;

/// Format pence as a plain decimal string, e.g. `-1234` -> `"-12.34"`.
pub fn to_decimal(pence: i64) -> String {
    let sign = if pence < 0 { "-" } else { "" };
    let abs = pence.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

/// A separator inside the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sep {
    Dot,
    Comma,
    Space,
    Apostrophe,
}

impl Sep {
    fn of(c: char) -> Option<Self> {
        match c {
            '.' => Some(Sep::Dot),
            ',' => Some(Sep::Comma),
            '\'' | '’' => Some(Sep::Apostrophe),
            c if c.is_whitespace() => Some(Sep::Space),
            _ => None,
        }
    }

    fn can_be_decimal(self) -> bool {
        matches!(self, Sep::Dot | Sep::Comma)
    }
}

/// Parse user input into pence. Accepts either `.` or `,` as the decimal
/// separator: the last `.`/`,` counts as the decimal point when 0–2 digits
/// follow it, otherwise it is a thousands separator. So `12,50`, `12.50`,
/// `£1,234.56`, `1.234,56 €`, `CHF 1'234.50` and `-3.20` all work in any locale.
///
/// Anything that isn't a well-formed number is rejected rather than guessed
/// at: `0.125` (three decimal places), `1e3`, `1.2.3` and `12 50` are errors.
/// A Unicode minus sign (`−`) counts as a minus sign.
pub fn parse(input: &str) -> Result<i64, String> {
    let text = input.trim();
    if text.is_empty() {
        return Err("Enter an amount".into());
    }
    let invalid = || format!("'{text}' is not a valid amount");
    let too_precise = || format!("'{text}' has too many decimal places (at most two)");

    let chars: Vec<char> = text.chars().map(|c| if c == '\u{2212}' { '-' } else { c }).collect();
    let first_digit = chars.iter().position(char::is_ascii_digit).ok_or_else(invalid)?;
    let last_digit = chars.iter().rposition(char::is_ascii_digit).ok_or_else(invalid)?;

    // A `.`/`,` right before the first digit (`.5`) or right after the last
    // (`12.`) belongs to the number.
    let mut start = first_digit;
    if start > 0 && matches!(chars[start - 1], '.' | ',') {
        start -= 1;
    }
    let mut end = last_digit + 1;
    if end < chars.len() && matches!(chars[end], '.' | ',') {
        end += 1;
    }

    // Around the number: an optional minus sign before it, and currency text.
    let (prefix, body, suffix) = (&chars[..start], &chars[start..end], &chars[end..]);
    let minus_signs = prefix.iter().filter(|c| **c == '-').count();
    if minus_signs > 1 || suffix.contains(&'-') {
        return Err(invalid());
    }
    let currency_len = |part: &[char]| part.iter().filter(|c| !c.is_whitespace() && **c != '-').count();
    if currency_len(prefix) > MAX_CURRENCY_CHARS || currency_len(suffix) > MAX_CURRENCY_CHARS {
        return Err(invalid());
    }
    let negative = minus_signs == 1;

    // Split the number into digit groups and the separators between them.
    let mut groups: Vec<String> = vec![String::new()];
    let mut seps: Vec<Sep> = Vec::new();
    for &c in body {
        if c.is_ascii_digit() {
            groups.last_mut().expect("never empty").push(c);
        } else if let Some(sep) = Sep::of(c) {
            // Two separators in a row (`1.,2`) are not a number, but a run of
            // whitespace counts as one separator. A leading `.`/`,` is `.5`.
            if groups.last().is_some_and(String::is_empty) && groups.len() > 1 {
                if sep == Sep::Space && seps.last() == Some(&Sep::Space) {
                    continue;
                }
                return Err(invalid());
            }
            seps.push(sep);
            groups.push(String::new());
        } else {
            return Err(invalid());
        }
    }

    // Find the decimal point, if any.
    let (int_groups, int_seps, frac) = match seps.last() {
        Some(&last) if last.can_be_decimal() => {
            let frac = groups.last().expect("never empty");
            match frac.len() {
                0..=2 => (&groups[..groups.len() - 1], &seps[..seps.len() - 1], frac.as_str()),
                3 => (&groups[..], &seps[..], ""),
                _ => return Err(too_precise()),
            }
        }
        _ => (&groups[..], &seps[..], ""),
    };

    // What's left is the whole part, optionally grouped in thousands with one
    // kind of separator, which can't also be the decimal point.
    if let Some(&group_sep) = int_seps.first() {
        let decimal = (int_seps.len() < seps.len()).then(|| seps[seps.len() - 1]);
        let consistent = int_seps.iter().all(|s| *s == group_sep) && decimal != Some(group_sep);
        let first = &int_groups[0];
        let (last, middle) = int_groups[1..].split_last().expect("a separator means two groups");
        // Middle groups may have two digits, for Indian grouping (1,23,456).
        let shaped = (1..=3).contains(&first.len())
            && !first.starts_with('0')
            && last.len() == 3
            && middle.iter().all(|g| (2..=3).contains(&g.len()));
        if !consistent || !shaped {
            // `0.125` and `1234.567` are three decimal places, not thousands.
            let three_places = decimal.is_none() && group_sep.can_be_decimal() && int_seps.len() == 1;
            return Err(if three_places { too_precise() } else { invalid() });
        }
    } else if int_groups[0].is_empty() && frac.is_empty() {
        return Err(invalid());
    }

    let whole: String = int_groups.concat();
    let whole: i64 = if whole.is_empty() { 0 } else { whole.parse().map_err(|_| "Amount is too large".to_string())? };
    let frac: i64 = format!("{frac:0<2}").parse().unwrap_or(0);
    let pence = whole
        .checked_mul(100)
        .and_then(|p| p.checked_add(frac))
        .filter(|p| *p <= MAX_PENCE)
        .ok_or_else(|| "Amount is too large".to_string())?;
    Ok(if negative { -pence } else { pence })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses() {
        assert_eq!(parse("12"), Ok(1200));
        assert_eq!(parse("12.5"), Ok(1250));
        assert_eq!(parse("£1,234.56"), Ok(123456));
        assert_eq!(parse("-3.20"), Ok(-320));
        assert_eq!(parse(".5"), Ok(50));
        assert_eq!(parse("-.5"), Ok(-50));
        assert_eq!(parse("12,50"), Ok(1250));
        assert_eq!(parse("1.234,56 €"), Ok(123456));
        assert_eq!(parse("1 234,5"), Ok(123450));
        assert_eq!(parse("1\u{a0}234,50 kr"), Ok(123450));
        assert_eq!(parse("1.234"), Ok(123400));
        assert_eq!(parse("1,234"), Ok(123400));
        assert_eq!(parse("12,345.6"), Ok(1234560));
        assert_eq!(parse("CHF 1'234.50"), Ok(123450));
        assert_eq!(parse("1,23,456"), Ok(12345600));
        assert_eq!(parse("12."), Ok(1200));
        assert_eq!(parse("-£3.20"), Ok(-320));
        assert_eq!(parse("£-3.20"), Ok(-320));
        assert_eq!(parse("−5"), Ok(-500), "Unicode minus sign");
        assert_eq!(parse("0.05"), Ok(5));
    }

    #[test]
    fn rejects() {
        assert_eq!(parse(""), Err("Enter an amount".into()));
        assert!(parse("abc").is_err());
        assert!(parse("-").is_err());
        assert!(parse(".").is_err());
        assert!(parse("1000000").is_err());
        assert!(parse("1-2").is_err());
        assert!(parse("--5").is_err());
        assert!(parse("5-").is_err());
        assert!(parse("1e3").is_err());
        assert!(parse("1.2.3").is_err());
        assert!(parse("1.234.56").is_err());
        assert!(parse("1.,2").is_err());
        assert!(parse("12 50").is_err());
        assert!(parse("1,2345").is_err());
        assert!(parse("1.234,567").is_err());
        assert!(parse("12 dollars").is_err());
        assert!(parse("0.125").unwrap_err().contains("decimal places"));
        assert!(parse("0,125").unwrap_err().contains("decimal places"));
        assert!(parse("1234.567").unwrap_err().contains("decimal places"));
        assert!(parse("1.2345").unwrap_err().contains("decimal places"));
    }

    #[test]
    fn formats() {
        assert_eq!(to_decimal(-1234), "-12.34");
        assert_eq!(to_decimal(5), "0.05");
    }
}
