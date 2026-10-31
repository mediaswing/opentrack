//! Amounts of money, calendar months, and the dates the Spending and
//! Summary tabs show.
//!
//! Amounts are kept as whole pence (or cents), never as floating point, so
//! totals always add up. The schema's `DECIMAL(8,2)` allows up to 999,999.99
//! either way.

use chrono::{Datelike, Days, Months, NaiveDate};
use serde::{Deserialize, Serialize};

/// The largest amount `DECIMAL(8,2)` holds, in pence: 999,999.99.
pub const MAX_PENCE: i64 = 99_999_999;

/// How a region writes numbers: the decimal mark and the thousands
/// separator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Numbers {
    pub decimal: char,
    pub group: char,
}

impl Numbers {
    /// As in files: "1,234.56".
    pub const PLAIN: Numbers = Numbers { decimal: '.', group: ',' };

    fn example(self) -> String {
        format!("12{}50", self.decimal)
    }
}

/// Where the currency symbol goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SymbolAt {
    /// "£1,234.56"
    Before,
    /// "€ 1.234,56"
    BeforeSpaced,
    /// "1.234,56 €"
    After,
}

/// Reads an amount such as "12.50", "£1,234", "-3.2" or "7", or "1.234,50"
/// where a comma is the decimal mark. A currency symbol, spaces and
/// thousands separators are ignored. A single mark followed by one or two
/// digits is taken as the decimal mark either way, so "8.50" and "8,50" are
/// the same everywhere.
pub fn parse(text: &str, numbers: Numbers) -> Result<i64, String> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '\'' | '’') && !is_currency_symbol(*c))
        .collect();
    if cleaned.is_empty() {
        return Err(t!("amount-missing", example = numbers.example()));
    }
    let other = if numbers.decimal == ',' { '.' } else { ',' };
    let decimal = if !cleaned.contains(numbers.decimal)
        && cleaned.matches(other).count() == 1
        && cleaned.rsplit(other).next().is_some_and(|f| (1..=2).contains(&f.len()))
    {
        other
    } else {
        numbers.decimal
    };
    let cleaned: String = cleaned
        .chars()
        .filter(|c| !matches!(c, '.' | ',') || *c == decimal)
        .map(|c| if c == decimal { '.' } else { c })
        .collect();
    let (negative, digits) = match cleaned.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, cleaned.as_str()),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let all_digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
    if (whole.is_empty() && fraction.is_empty()) || !all_digits(whole) || !all_digits(fraction) {
        return Err(t!("amount-invalid", text = text.trim(), example = numbers.example()));
    }
    if fraction.len() > 2 {
        return Err(t!("amount-too-precise"));
    }
    let too_large = || t!("amount-too-large", max = format(MAX_PENCE, "", numbers, SymbolAt::Before));
    if whole.len() > 6 {
        return Err(too_large());
    }
    let whole: i64 = if whole.is_empty() { 0 } else { whole.parse().map_err(|_| too_large())? };
    let fraction: i64 = format!("{fraction:0<2}").parse().unwrap_or(0);
    let pence = whole * 100 + fraction;
    Ok(if negative { -pence } else { pence })
}

fn is_currency_symbol(c: char) -> bool {
    matches!(c, '£' | '$' | '€' | '¥' | '₹' | '₩' | '₽' | '₺' | '₪' | '₫' | '₴' | '¢')
}

/// An amount as shown, such as "£1,234.56", "-£3.00" or "1.234,56 €".
pub fn format(pence: i64, symbol: &str, numbers: Numbers, at: SymbolAt) -> String {
    let sign = if pence < 0 { "-" } else { "" };
    let abs = pence.unsigned_abs();
    let whole = (abs / 100).to_string();
    let mut grouped = String::new();
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            grouped.push(numbers.group);
        }
        grouped.push(c);
    }
    let number = format!("{grouped}{}{:02}", numbers.decimal, abs % 100);
    // A symbol made of letters, such as "kr" or "CHF", needs a space.
    let spaced = at == SymbolAt::BeforeSpaced || symbol.chars().last().is_some_and(char::is_alphabetic);
    match at {
        _ if symbol.is_empty() => format!("{sign}{number}"),
        SymbolAt::After => format!("{sign}{number}\u{a0}{symbol}"),
        _ if spaced => format!("{sign}{symbol}\u{a0}{number}"),
        _ => format!("{sign}{symbol}{number}"),
    }
}

/// An amount as put in a box to be changed, such as "1234.56" or "1234,56".
pub fn editable(pence: i64, numbers: Numbers) -> String {
    plain(pence).replace('.', &numbers.decimal.to_string())
}

/// An amount as written in exported files, such as "1234.56" or "-3.00".
pub fn plain(pence: i64) -> String {
    let sign = if pence < 0 { "-" } else { "" };
    let abs = pence.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

/// Reads a date written as YYYY-MM-DD, as the database stores them.
pub fn parse_date(text: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d")
        .map_err(|_| t!("date-invalid-file", text = text.trim()))
}

/// A month's short name, such as "Oct", from its number, 1 to 12.
pub fn short_month_name(month: u32) -> String {
    crate::i18n::message(&format!("month-short-{month}"), None)
}

pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// A calendar month.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Month {
    pub year: i32,
    /// 1 to 12.
    pub month: u32,
}

impl Month {
    pub fn of(date: NaiveDate) -> Self {
        Month { year: date.year(), month: date.month() }
    }

    pub fn current() -> Self {
        Month::of(today())
    }

    pub fn first_day(self) -> NaiveDate {
        NaiveDate::from_ymd_opt(self.year, self.month, 1).expect("months are always 1 to 12")
    }

    pub fn next(self) -> Self {
        if self.month == 12 { Month { year: self.year + 1, month: 1 } } else { Month { month: self.month + 1, ..self } }
    }

    pub fn previous(self) -> Self {
        if self.month == 1 { Month { year: self.year - 1, month: 12 } } else { Month { month: self.month - 1, ..self } }
    }

    /// Moves `count` months, backwards if negative.
    pub fn plus(self, count: i32) -> Self {
        let index = self.year * 12 + self.month as i32 - 1 + count;
        Month { year: index.div_euclid(12), month: index.rem_euclid(12) as u32 + 1 }
    }

    /// As shown, such as "October 2026".
    pub fn label(self) -> String {
        let month = crate::i18n::message(&format!("month-{}", self.month), None);
        t!("month-year", month = month, year = self.year.to_string())
    }

    /// As shown under a chart column, such as "Oct".
    pub fn short_label(self) -> String {
        short_month_name(self.month)
    }

    /// As stored in the database, such as "2026-10", for matching dates.
    pub fn key(self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }

    pub fn last_day(self) -> NaiveDate {
        self.next().first_day().pred_opt().expect("a month always has a last day")
    }

    fn days(self) -> i64 {
        (self.next().first_day() - self.first_day()).num_days()
    }
}

/// The dates the Spending and Summary tabs show: a calendar month, or any
/// run of days, such as a financial year.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Period {
    Month(Month),
    /// From `first` to `last`, both included.
    Days { first: NaiveDate, last: NaiveDate },
}

impl Period {
    pub fn first_day(self) -> NaiveDate {
        match self {
            Period::Month(m) => m.first_day(),
            Period::Days { first, .. } => first,
        }
    }

    pub fn last_day(self) -> NaiveDate {
        match self {
            Period::Month(m) => m.last_day(),
            Period::Days { last, .. } => last,
        }
    }

    /// The day after the last, as an end that isn't included.
    pub fn end(self) -> NaiveDate {
        self.last_day().succ_opt().unwrap_or(NaiveDate::MAX)
    }

    pub fn contains(self, date: NaiveDate) -> bool {
        (self.first_day()..self.end()).contains(&date)
    }

    /// Every month with a day in the period, oldest first.
    pub fn months(self) -> Vec<Month> {
        let (first, last) = (Month::of(self.first_day()), Month::of(self.last_day()));
        std::iter::successors(Some(first), |m| Some(m.next())).take_while(|m| *m <= last).collect()
    }

    /// The period before, of the same length. A run of whole months, such
    /// as 6 April to 5 April, moves by months, so it stays in step with the
    /// calendar; any other run of days moves by its number of days. So does
    /// a run starting after the 28th, which a shorter month would move to
    /// an earlier day, so that moving back would not undo moving on.
    pub fn previous(self) -> Self {
        self.step(false)
    }

    pub fn next(self) -> Self {
        self.step(true)
    }

    fn step(self, forward: bool) -> Self {
        let (first, last) = match self {
            Period::Month(m) => return Period::Month(if forward { m.next() } else { m.previous() }),
            Period::Days { first, last } => (first, last),
        };
        let end = self.end();
        let months = (Month::of(end).year - Month::of(first).year) * 12 + end.month() as i32 - first.month() as i32;
        let moved = if end.day() == first.day() && first.day() <= 28 && months > 0 {
            let by = Months::new(months as u32);
            let first = if forward { first.checked_add_months(by) } else { first.checked_sub_months(by) };
            first.and_then(|f| Some((f, f.checked_add_months(by)?.pred_opt()?)))
        } else {
            let by = Days::new((last - first).num_days() as u64 + 1);
            if forward {
                first.checked_add_days(by).zip(last.checked_add_days(by))
            } else {
                first.checked_sub_days(by).zip(last.checked_sub_days(by))
            }
        };
        moved.map_or(self, |(first, last)| Period::Days { first, last })
    }

    /// A monthly budget for the period: the whole budget for each month it
    /// covers, and a share by days of the months it covers in part.
    pub fn budget(self, monthly: i64) -> i64 {
        self.months()
            .into_iter()
            .map(|m| {
                let first = self.first_day().max(m.first_day());
                let last = self.last_day().min(m.last_day());
                let days = (last - first).num_days() + 1;
                // Rounded to the nearest penny.
                (monthly * days * 2 + m.days()).div_euclid(m.days() * 2)
            })
            .sum()
    }
}

/// The day a financial year starts, such as 6 April.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct YearStart {
    /// 1 to 12.
    pub month: u32,
    /// 1 to 31. In a shorter month, the year starts on its last day.
    pub day: u32,
}

impl YearStart {
    pub fn is_valid(self) -> bool {
        (1..=12).contains(&self.month) && (1..=31).contains(&self.day)
    }

    fn in_year(self, year: i32) -> NaiveDate {
        let month = Month { year, month: self.month };
        let last = month.last_day().day();
        NaiveDate::from_ymd_opt(year, self.month, self.day.min(last)).unwrap_or(month.first_day())
    }

    /// The financial year that `date` is in, from its first day to its last.
    pub fn year_of(self, date: NaiveDate) -> Period {
        let year = if date >= self.in_year(date.year()) { date.year() } else { date.year() - 1 };
        let first = self.in_year(year);
        let last = self.in_year(year + 1).pred_opt().unwrap_or(first);
        Period::Days { first, last }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMA: Numbers = Numbers { decimal: ',', group: '.' };

    #[test]
    fn parses_amounts() {
        let parse = |text| parse(text, Numbers::PLAIN);
        assert_eq!(parse("12.50"), Ok(1250));
        assert_eq!(parse("£1,234"), Ok(123_400));
        assert_eq!(parse(" -3.2 "), Ok(-320));
        assert_eq!(parse(".5"), Ok(50));
        assert_eq!(parse("8,50"), Ok(850));
        assert_eq!(parse("999999.99"), Ok(MAX_PENCE));
        assert!(parse("").is_err());
        assert!(parse("1.234").is_err());
        assert!(parse("1000000").is_err());
        assert!(parse("12a").is_err());
        assert!(parse("-").is_err());
        assert!(parse("1.2.3").is_err());
    }

    #[test]
    fn parses_amounts_with_a_decimal_comma() {
        let parse = |text| parse(text, COMMA);
        assert_eq!(parse("12,50"), Ok(1250));
        assert_eq!(parse("1.234,5 €"), Ok(123_450));
        assert_eq!(parse("1.234"), Ok(123_400));
        assert_eq!(parse("8.50"), Ok(850));
        assert_eq!(parse("1\u{a0}234,56"), Ok(123_456));
        assert_eq!(parse("1’234.56"), Ok(123_456));
        assert!(parse("1,234").is_err());
        assert_eq!(parse("x"), Err("\"x\" is not an amount. Type one such as 12,50.".into()));
    }

    #[test]
    fn formats_amounts() {
        let at = SymbolAt::Before;
        assert_eq!(format(123_456, "£", Numbers::PLAIN, at), "£1,234.56");
        assert_eq!(format(-300, "£", Numbers::PLAIN, at), "-£3.00");
        assert_eq!(format(5, "$", Numbers::PLAIN, at), "$0.05");
        assert_eq!(format(100_000_000, "", Numbers::PLAIN, at), "1,000,000.00");
        assert_eq!(format(123_456, "€", COMMA, SymbolAt::After), "1.234,56\u{a0}€");
        assert_eq!(format(123_456, "€", COMMA, SymbolAt::BeforeSpaced), "€\u{a0}1.234,56");
        assert_eq!(format(500, "CHF", Numbers::PLAIN, at), "CHF\u{a0}5.00");
        assert_eq!(plain(-1205), "-12.05");
        assert_eq!(editable(-1205, COMMA), "-12,05");
        for pence in [0, 1, 99, 100, -1, 123_456, -MAX_PENCE] {
            for numbers in [Numbers::PLAIN, COMMA] {
                for at in [SymbolAt::Before, SymbolAt::After] {
                    assert_eq!(parse(&format(pence, "£", numbers, at), numbers), Ok(pence));
                }
                assert_eq!(parse(&editable(pence, numbers), numbers), Ok(pence));
            }
            assert_eq!(parse(&plain(pence), Numbers::PLAIN), Ok(pence));
        }
    }

    #[test]
    fn months() {
        let dec = Month { year: 2025, month: 12 };
        assert_eq!(dec.next(), Month { year: 2026, month: 1 });
        assert_eq!(dec.next().previous(), dec);
        assert_eq!(dec.plus(-12), Month { year: 2024, month: 12 });
        assert_eq!(dec.plus(-11), Month { year: 2025, month: 1 });
        assert_eq!(dec.plus(13), Month { year: 2027, month: 1 });
        assert_eq!(dec.key(), "2025-12");
        assert_eq!(dec.label(), "December 2025");
    }

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn periods() {
        let oct = Period::Month(Month { year: 2026, month: 10 });
        assert_eq!((oct.first_day(), oct.last_day(), oct.end()), (day(2026, 10, 1), day(2026, 10, 31), day(2026, 11, 1)));
        assert_eq!(oct.budget(20_000), 20_000);
        assert_eq!(oct.next(), Period::Month(Month { year: 2026, month: 11 }));

        let tax = YearStart { month: 4, day: 6 };
        let year = tax.year_of(day(2026, 10, 31));
        assert_eq!(year, Period::Days { first: day(2026, 4, 6), last: day(2027, 4, 5) });
        assert_eq!(tax.year_of(day(2026, 4, 5)).first_day(), day(2025, 4, 6));
        assert_eq!(tax.year_of(day(2026, 4, 6)).first_day(), day(2026, 4, 6));
        assert_eq!(year.months().len(), 13);
        assert_eq!(year.next(), tax.year_of(day(2027, 4, 6)), "a year moves by twelve months");
        assert_eq!(year.previous(), tax.year_of(day(2025, 4, 6)));
        assert!(year.contains(day(2027, 4, 5)) && !year.contains(day(2027, 4, 6)));
        // 25/30 of April, eleven whole months, and 5/30 of the next April.
        assert_eq!(year.budget(3000), 2500 + 11 * 3000 + 500);
        assert_eq!(YearStart { month: 1, day: 1 }.year_of(day(2026, 10, 31)).budget(3000), 12 * 3000);
        let leap = YearStart { month: 2, day: 30 }.year_of(day(2028, 3, 1));
        assert_eq!(leap.first_day(), day(2028, 2, 29), "a day past the month's end is its last day");

        let week = Period::Days { first: day(2026, 10, 29), last: day(2026, 11, 4) };
        assert_eq!(week.next(), Period::Days { first: day(2026, 11, 5), last: day(2026, 11, 11) });
        assert_eq!(week.previous().next(), week);
        // Starting on the 31st, a month back can only land on the 30th.
        let late = Period::Days { first: day(2026, 1, 31), last: day(2026, 3, 30) };
        assert_eq!(late.previous().next(), late, "Earlier then Later comes back to the same dates");
        assert_eq!(late.next().previous(), late);
        // 3 of October's 31 days and 4 of November's 30.
        assert_eq!(week.budget(31_000), 3000 + 4133);
    }

    #[test]
    fn dates() {
        assert_eq!(parse_date("2026-10-30"), Ok(NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()));
        assert!(parse_date("30/10/2026").is_err());
        assert!(parse_date("2026-02-30").is_err());
    }
}
