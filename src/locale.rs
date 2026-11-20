//! Locale detection and locale-aware formatting of dates and money.
//!
//! The locale is chosen, in order, from: the `--locale` option, the
//! `OPENTRACK_LOCALE` environment variable, the POSIX variables `LC_ALL`,
//! `LC_MONETARY`, `LC_TIME` and `LANG`, and finally the operating system's
//! preferred locale. If none of these name a known locale, `en_US` is used.
//!
//! Formatting rules (currency symbol and position, decimal and thousands
//! separators, digit grouping, date order, and month and day names) come from
//! the glibc locale database bundled in the `pure-rust-locales` crate, which
//! covers several hundred locales.

use chrono::{Datelike, Duration, Locale, NaiveDate};
use pure_rust_locales::locale_match;

use crate::money::{self, ParseError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateOrder {
    DayMonthYear,
    MonthDayYear,
    YearMonthDay,
}

#[derive(Debug, Clone)]
pub struct LocaleFormat {
    pub locale: Locale,
    /// Where the locale came from, for the debug log and `--help`.
    pub source: String,
    currency: String,
    symbol_first: bool,
    symbol_space: bool,
    frac_digits: u32,
    decimal: String,
    thousands: String,
    grouping: Vec<i64>,
    pub date_order: DateOrder,
    short_date_fmt: String,
    long_date_fmt: String,
}

const FALLBACK: &str = "en_US";

impl LocaleFormat {
    /// Detect the locale from the environment. `explicit` is the `--locale`
    /// option; if it is given but unknown, an error is returned.
    pub fn detect(explicit: Option<&str>) -> Result<Self, String> {
        if let Some(name) = explicit {
            return resolve_explicit(name)
                .map(|l| Self::new(l, "--locale option"))
                .ok_or_else(|| format!("Unknown locale \"{name}\". Use a name such as en_GB or de_DE."));
        }
        let mut candidates: Vec<(String, String)> = Vec::new();
        for var in ["OPENTRACK_LOCALE", "LC_ALL", "LC_MONETARY", "LC_TIME", "LANG"] {
            if let Ok(v) = std::env::var(var) {
                candidates.push((format!("{var} environment variable"), v));
            }
        }
        if let Some(v) = sys_locale::get_locale() {
            candidates.push(("operating system".into(), v));
        }
        for (source, value) in candidates {
            match resolve(&value) {
                Some(l) => {
                    log::debug!("locale candidate {value:?} from {source}: using {l}");
                    return Ok(Self::new(l, &source));
                }
                None => log::debug!("locale candidate {value:?} from {source}: not usable, skipped"),
            }
        }
        log::warn!("no usable locale found, falling back to {FALLBACK}");
        Ok(Self::new(Locale::try_from(FALLBACK).unwrap(), "fallback default"))
    }

    pub fn new(locale: Locale, source: &str) -> Self {
        let symbol = locale_match!(locale => LC_MONETARY::CURRENCY_SYMBOL);
        let int_symbol = locale_match!(locale => LC_MONETARY::INT_CURR_SYMBOL).trim();
        let currency = if symbol.is_empty() { int_symbol } else { symbol }.to_string();
        let frac = locale_match!(locale => LC_MONETARY::FRAC_DIGITS);
        let decimal = locale_match!(locale => LC_MONETARY::MON_DECIMAL_POINT);
        let d_fmt = locale_match!(locale => LC_TIME::D_FMT);
        let (date_order, separator) = date_order_of(d_fmt);
        let short_date_fmt = numeric_date_format(d_fmt, date_order);
        let long_date_fmt = match (date_order, separator) {
            (DateOrder::DayMonthYear, '.') => "%A, %-d. %B %Y".to_string(),
            (DateOrder::DayMonthYear, _) => "%A %-d %B %Y".to_string(),
            (DateOrder::MonthDayYear, _) => "%A, %B %-d, %Y".to_string(),
            (DateOrder::YearMonthDay, s) if s.is_ascii() => "%A %-d %B %Y".to_string(),
            (DateOrder::YearMonthDay, _) => format!("%A {short_date_fmt}"),
        };
        Self {
            locale,
            source: source.to_string(),
            currency,
            symbol_first: locale_match!(locale => LC_MONETARY::P_CS_PRECEDES) == 1,
            symbol_space: locale_match!(locale => LC_MONETARY::P_SEP_BY_SPACE) >= 1,
            frac_digits: if frac < 0 { 2 } else { frac.min(3) as u32 },
            decimal: if decimal.is_empty() { ".".into() } else { decimal.into() },
            thousands: locale_match!(locale => LC_MONETARY::MON_THOUSANDS_SEP).to_string(),
            grouping: locale_match!(locale => LC_MONETARY::MON_GROUPING).to_vec(),
            date_order,
            short_date_fmt,
            long_date_fmt,
        }
    }

    // ----- money -----------------------------------------------------------

    /// Format an amount in hundredths for display, for example `£1,234.56`,
    /// `1.234,56 €` or `minus $3.99`. "minus" is used instead of a hyphen
    /// because many screen readers skip or mispronounce a leading hyphen.
    pub fn money(&self, hundredths: i64) -> String {
        let sign = if hundredths < 0 { "minus " } else { "" };
        let number = self.number(hundredths.unsigned_abs());
        let space = if self.symbol_space { " " } else { "" };
        match (self.currency.is_empty(), self.symbol_first) {
            (true, _) => format!("{sign}{number}"),
            (false, true) => format!("{sign}{}{space}{number}", self.currency),
            (false, false) => format!("{sign}{number}{space}{}", self.currency),
        }
    }

    /// The number part only, with locale separators.
    fn number(&self, hundredths: u64) -> String {
        let (whole, frac) = match self.frac_digits {
            0 => ((hundredths + 50) / 100, String::new()),
            1 => (hundredths / 100, format!("{}", hundredths % 100 / 10)),
            n => (hundredths / 100, format!("{:02}{}", hundredths % 100, "0".repeat(n as usize - 2))),
        };
        let mut out = self.group(whole);
        if !frac.is_empty() {
            out.push_str(&self.decimal);
            out.push_str(&frac);
        }
        out
    }

    /// Insert thousands separators following the locale's grouping rule,
    /// e.g. [3, 3] gives 1,234,567 and [3, 2] gives 12,34,567.
    fn group(&self, whole: u64) -> String {
        let digits = whole.to_string();
        if self.thousands.is_empty() || self.grouping.is_empty() {
            return digits;
        }
        let mut groups: Vec<&str> = Vec::new();
        let mut end = digits.len();
        let mut sizes = self.grouping.iter();
        let mut size = *sizes.next().unwrap();
        while end > 0 {
            if size <= 0 || size >= 127 {
                groups.push(&digits[..end]);
                break;
            }
            let start = end.saturating_sub(size as usize);
            groups.push(&digits[start..end]);
            end = start;
            if let Some(next) = sizes.next() {
                size = *next;
            }
        }
        groups.reverse();
        groups.join(&self.thousands)
    }

    /// An example amount to show in prompts, such as `12.50` or `12,50`.
    pub fn example_amount(&self) -> String {
        self.number(1250)
    }

    /// Parse an amount typed by the user. Accepts the locale's own separators
    /// and currency symbol. If the locale uses a comma for decimals but the
    /// user types `12.50` (or the other way round), that is understood too,
    /// as long as it cannot be a thousands separator.
    pub fn parse_money(&self, input: &str) -> Result<i64, String> {
        let decimal = self.decimal.chars().next().unwrap_or('.');
        let other = if decimal == '.' { ',' } else { '.' };
        let mut s: &str = input.trim();
        let mut negative = false;
        for prefix in ["minus ", "-"] {
            if s.get(..prefix.len()).is_some_and(|p| p.eq_ignore_ascii_case(prefix)) {
                negative = true;
                s = &s[prefix.len()..];
                break;
            }
        }
        let mut s = s.to_string();
        for sym in [self.currency.as_str(), locale_match!(self.locale => LC_MONETARY::INT_CURR_SYMBOL).trim()] {
            if !sym.is_empty() {
                s = s.replace(sym, "");
            }
        }
        let mut s: String = s
            .chars()
            .filter(|c| !c.is_whitespace() && !matches!(c, '£' | '$' | '€' | '¥' | '\u{a0}' | '\u{202f}'))
            .collect();
        if !s.contains(decimal) && s.matches(other).count() == 1 {
            let tail = s.rsplit(other).next().unwrap_or("");
            if (1..=2).contains(&tail.len()) {
                s = s.replace(other, &decimal.to_string());
            }
        }
        let canonical: String = s
            .chars()
            .filter(|c| *c != other && !self.thousands.contains(*c) || *c == decimal)
            .map(|c| if c == decimal { '.' } else { c })
            .collect();
        // Currencies with three decimal places show amounts such as 12.500,
        // as the example in the Amount box does, but only hundredths are
        // kept, so a final zero can go.
        let canonical = match canonical.split_once('.') {
            Some((whole, frac)) if self.frac_digits > 2 && frac.len() == 3 && frac.ends_with('0') => {
                format!("{whole}.{}", &frac[..2])
            }
            _ => canonical,
        };
        let canonical = if negative { format!("-{canonical}") } else { canonical };
        money::parse_canonical(&canonical).map_err(|e| match e {
            ParseError::NotANumber => format!(
                "\"{}\" is not an amount. Type a number such as {}.",
                input.trim(),
                self.example_amount()
            ),
            ParseError::TooLarge => format!(
                "That amount is too large. The most allowed is {}.",
                self.money(money::MAX_HUNDREDTHS)
            ),
        })
    }

    // ----- dates -----------------------------------------------------------

    /// A date written out for reading aloud, such as "Wednesday 18 November 2026".
    pub fn long_date(&self, d: NaiveDate) -> String {
        d.format_localized(&self.long_date_fmt, self.locale).to_string()
    }

    /// A short numeric date in the locale's order, such as 18/11/2026.
    pub fn short_date(&self, d: NaiveDate) -> String {
        d.format_localized(&self.short_date_fmt, self.locale).to_string()
    }

    /// A month heading, such as "November 2026".
    pub fn month_year(&self, d: NaiveDate) -> String {
        d.format_localized("%B %Y", self.locale).to_string()
    }

    pub fn describe(&self) -> String {
        format!(
            "{} (from the {}): dates like {}, money like {}",
            self.locale,
            self.source,
            self.short_date(NaiveDate::from_ymd_opt(2026, 11, 18).unwrap()),
            self.money(123456)
        )
    }
}

/// Work out the order of day, month and year, and the separator, from a
/// strftime pattern such as "%d/%m/%Y".
fn date_order_of(d_fmt: &str) -> (DateOrder, char) {
    let mut order = Vec::new();
    let mut separator = None;
    let mut chars = d_fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            while let Some('-' | '_' | '0' | 'E' | 'O') = chars.peek() {
                chars.next();
            }
            match chars.next() {
                Some('d' | 'e') => order.push('d'),
                Some('m' | 'b' | 'B') => order.push('m'),
                Some('y' | 'Y' | 'C') => order.push('y'),
                _ => {}
            }
        } else if !order.is_empty() && separator.is_none() {
            separator = Some(c);
        }
    }
    let order = match order.as_slice() {
        ['m', 'd', ..] => DateOrder::MonthDayYear,
        ['y', ..] => DateOrder::YearMonthDay,
        _ => DateOrder::DayMonthYear,
    };
    (order, separator.unwrap_or('/'))
}

/// Parse a typed date. Numbers may be separated by any non-digit character. A
/// four-digit first number means year first; otherwise the locale's order is
/// used. A missing year means the current year, and empty means today.
pub fn parse_date(text: &str, today: NaiveDate, order: DateOrder) -> Result<NaiveDate, ()> {
    let t = text.trim().to_lowercase();
    match t.as_str() {
        "" | "today" => return Ok(today),
        "yesterday" => return Ok(today - Duration::days(1)),
        _ => {}
    }
    // Words such as "soon" are not dates; non-Latin date markers (年, 月) are fine.
    if t.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err(());
    }
    let parts: Vec<&str> = t.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).collect();
    // Anything longer than four digits can't be part of a date.
    if parts.iter().any(|p| p.len() > 4) {
        return Err(());
    }
    let nums: Vec<i32> = parts.iter().map(|p| p.parse().map_err(|_| ())).collect::<Result<_, _>>()?;
    let year = |y: i32, digits: usize| if digits <= 2 { 2000 + y } else { y };
    let (y, m, d) = match (nums.as_slice(), order) {
        ([y, m, d], _) if parts[0].len() == 4 => (*y, *m, *d),
        ([a, b, c], DateOrder::YearMonthDay) => (year(*a, parts[0].len()), *b, *c),
        ([a, b, c], DateOrder::MonthDayYear) => (year(*c, parts[2].len()), *a, *b),
        ([a, b, c], DateOrder::DayMonthYear) => (year(*c, parts[2].len()), *b, *a),
        ([a, b], DateOrder::DayMonthYear) => (today.year(), *b, *a),
        ([a, b], _) => (today.year(), *a, *b),
        _ => return Err(()),
    };
    let (m, d) = (u32::try_from(m).map_err(|_| ())?, u32::try_from(d).map_err(|_| ())?);
    NaiveDate::from_ymd_opt(y, m, d).ok_or(())
}

/// The locale's short date pattern with a four-digit year, if it is made of
/// day, month and year numbers alone. Some locales' patterns have month or
/// weekday names, such as "%d %b, %Y", and a date typed like that couldn't be
/// read back, so they get numbers in the same order instead. Non-Latin
/// markers, as in "%Y年%m月%d日", are kept, since they can be typed back.
fn numeric_date_format(d_fmt: &str, order: DateOrder) -> String {
    let pattern = d_fmt.replace("%y", "%Y").replace("%e", "%d");
    let mut chars = pattern.chars();
    let mut numeric = true;
    while let Some(c) = chars.next() {
        match c {
            '%' => numeric &= matches!(chars.next(), Some('d' | 'm' | 'Y')),
            c => numeric &= !c.is_ascii_alphabetic(),
        }
    }
    if numeric {
        return pattern;
    }
    match order {
        DateOrder::DayMonthYear => "%d/%m/%Y",
        DateOrder::MonthDayYear => "%m/%d/%Y",
        DateOrder::YearMonthDay => "%Y-%m-%d",
    }
    .to_owned()
}

/// Turn a locale name someone typed into a known locale. Unlike `resolve`, a
/// region that doesn't exist is a mistake to report, not a reason to guess:
/// "de_DE" and "de-de" work, as does "de" alone, but "de_XX" doesn't.
fn resolve_explicit(raw: &str) -> Option<Locale> {
    let base = raw.split(['.', '@']).next()?.trim().replace('-', "_");
    match base.split('_').collect::<Vec<_>>().as_slice() {
        [lang] => resolve(lang),
        [lang, region] => Locale::try_from(format!("{}_{}", lang.to_lowercase(), region.to_uppercase()).as_str()).ok(),
        _ => resolve(&base),
    }
}

/// Turn a locale string from the environment ("en_GB.UTF-8", "en-GB",
/// "zh-Hans-CN", "de") into a known locale.
fn resolve(raw: &str) -> Option<Locale> {
    let base = raw.split(['.', '@']).next()?.trim().replace('-', "_");
    if base.is_empty() || base == "C" || base == "POSIX" {
        return None;
    }
    if let Ok(l) = Locale::try_from(base.as_str()) {
        return Some(l);
    }
    let parts: Vec<&str> = base.split('_').collect();
    let lang = parts[0].to_lowercase();
    let region = parts[1..]
        .iter()
        .rev()
        .find(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic()))
        .map(|p| p.to_uppercase());
    let default_region: String = match lang.as_str() {
        "en" => "US",
        "ja" => "JP",
        "zh" => "CN",
        "ko" => "KR",
        "sv" => "SE",
        "da" => "DK",
        "nb" | "no" => "NO",
        "cs" => "CZ",
        "el" => "GR",
        "uk" => "UA",
        "ga" | "cy" | "gd" => "GB",
        other => return try_region(&lang, region, other.to_uppercase()),
    }
    .to_string();
    try_region(&lang, region, default_region)
}

fn try_region(lang: &str, region: Option<String>, default_region: String) -> Option<Locale> {
    for r in region.into_iter().chain([default_region]) {
        if let Ok(l) = Locale::try_from(format!("{lang}_{r}").as_str()) {
            return Some(l);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(name: &str) -> LocaleFormat {
        LocaleFormat::new(Locale::try_from(name).unwrap(), "test")
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn resolves_environment_strings() {
        assert_eq!(resolve("en_GB.UTF-8"), Some(Locale::en_GB));
        assert_eq!(resolve("en-GB"), Some(Locale::en_GB));
        assert_eq!(resolve("de"), Some(Locale::de_DE));
        assert_eq!(resolve("en"), Some(Locale::en_US));
        assert_eq!(resolve("zh-Hans-CN"), Some(Locale::zh_CN));
        assert_eq!(resolve("C"), None);
        assert_eq!(resolve("C.UTF-8"), None);
        assert_eq!(resolve("xx_YY"), None);
    }

    #[test]
    fn typed_locales_must_exist() {
        assert_eq!(resolve_explicit("de_DE"), Some(Locale::de_DE));
        assert_eq!(resolve_explicit("de-de"), Some(Locale::de_DE));
        assert_eq!(resolve_explicit("de"), Some(Locale::de_DE));
        assert_eq!(resolve_explicit("en_GB.UTF-8"), Some(Locale::en_GB));
        assert_eq!(resolve_explicit("de_DEx"), None);
        assert_eq!(resolve_explicit("de_XX"), None);
        assert!(LocaleFormat::detect(Some("de_XX")).is_err());
    }

    #[test]
    fn money_formats() {
        assert_eq!(fmt("en_GB").money(123456), "£1,234.56");
        assert_eq!(fmt("en_GB").money(-399), "minus £3.99");
        assert_eq!(fmt("en_US").money(123456), "$1,234.56");
        assert_eq!(fmt("de_DE").money(123456), "1.234,56 €");
        assert_eq!(fmt("ja_JP").money(123456), "￥1,235");
        assert_eq!(fmt("en_IN").money(123456789), "₹12,34,567.89");
    }

    #[test]
    fn money_parses() {
        let gb = fmt("en_GB");
        assert_eq!(gb.parse_money("£1,234.56"), Ok(123456));
        assert_eq!(gb.parse_money("12.5"), Ok(1250));
        assert_eq!(gb.parse_money("12,50"), Ok(1250));
        assert_eq!(gb.parse_money("1,234"), Ok(123400));
        assert_eq!(gb.parse_money("-3.99"), Ok(-399));
        assert_eq!(gb.parse_money("minus 3.99"), Ok(-399));
        assert_eq!(gb.parse_money("Minus 3.99"), Ok(-399));
        assert!(gb.parse_money("-İ5").is_err());
        assert!(gb.parse_money("İİminus 5").is_err());
        assert!(gb.parse_money("abc").is_err());
        assert!(gb.parse_money("1000000").is_err());

        let de = fmt("de_DE");
        assert_eq!(de.parse_money("1.234,56 €"), Ok(123456));
        assert_eq!(de.parse_money("12,5"), Ok(1250));
        assert_eq!(de.parse_money("12.50"), Ok(1250));
        assert_eq!(de.parse_money("1.234"), Ok(123400));
    }

    #[test]
    fn dates() {
        let wed = d(2026, 11, 18);
        assert_eq!(fmt("en_GB").long_date(wed), "Wednesday 18 November 2026");
        assert_eq!(fmt("en_GB").short_date(wed), "18/11/2026");
        assert_eq!(fmt("en_US").long_date(wed), "Wednesday, November 18, 2026");
        assert_eq!(fmt("en_US").short_date(wed), "11/18/2026");
        assert_eq!(fmt("de_DE").long_date(wed), "Mittwoch, 18. November 2026");
        assert_eq!(fmt("fr_FR").month_year(wed), "novembre 2026");
        assert_eq!(fmt("en_US").date_order, DateOrder::MonthDayYear);
        assert_eq!(fmt("sv_SE").date_order, DateOrder::YearMonthDay);
        assert_eq!(fmt("ja_JP").short_date(wed), "2026年11月18日");
    }

    /// The date given as an example in the Date box can be typed back, in
    /// every locale, including those whose own pattern has month names.
    #[test]
    fn example_dates_can_be_typed_back() {
        let wed = d(2026, 11, 18);
        for name in ["ar_SA", "ar_EG", "en_GB", "en_US", "de_DE", "ja_JP", "ko_KR", "eu_ES", "sv_SE", "fa_IR", "ps_AF"] {
            let f = fmt(name);
            let example = f.short_date(wed);
            assert_eq!(
                parse_date(&example, wed, f.date_order),
                Ok(wed),
                "{name} shows {example:?}, which must read back"
            );
        }
        assert_eq!(fmt("ar_SA").short_date(wed), "18/11/2026");
    }

    /// Currencies with three decimal places accept the amount they show.
    #[test]
    fn three_decimal_currencies_accept_their_example() {
        let kw = fmt("ar_KW");
        assert_eq!(kw.example_amount().chars().filter(|c| c.is_ascii_digit()).count(), 5, "{}", kw.example_amount());
        assert_eq!(kw.parse_money(&kw.example_amount()), Ok(1250));
        assert!(kw.parse_money("12.345").is_err(), "only hundredths are kept");
    }

    #[test]
    fn typed_dates() {
        let today = d(2026, 11, 18);
        let dmy = DateOrder::DayMonthYear;
        let mdy = DateOrder::MonthDayYear;
        assert_eq!(parse_date("", today, dmy), Ok(today));
        assert_eq!(parse_date("Yesterday", today, dmy), Ok(d(2026, 11, 17)));
        assert_eq!(parse_date("2026-01-05", today, mdy), Ok(d(2026, 1, 5)));
        assert_eq!(parse_date("5/1/2026", today, dmy), Ok(d(2026, 1, 5)));
        assert_eq!(parse_date("5/1/2026", today, mdy), Ok(d(2026, 5, 1)));
        assert_eq!(parse_date("5.1.26", today, dmy), Ok(d(2026, 1, 5)));
        assert_eq!(parse_date("5/1", today, dmy), Ok(d(2026, 1, 5)));
        assert_eq!(parse_date("5/1", today, mdy), Ok(d(2026, 5, 1)));
        assert_eq!(parse_date("2026年11月18日", today, DateOrder::YearMonthDay), Ok(today));
        assert_eq!(parse_date("26-11-18", today, DateOrder::YearMonthDay), Ok(today));
        assert!(parse_date("18/4294967297/2026", today, dmy).is_err());
        assert!(parse_date("31/02/2026", today, dmy).is_err());
        assert!(parse_date("soon", today, dmy).is_err());
        assert!(parse_date("5", today, dmy).is_err());
    }
}
