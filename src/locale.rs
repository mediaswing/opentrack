//! The user's region: how dates and amounts are written, and the usual
//! currency symbol. It is found from the system's region settings (the
//! regional format on Windows, the Region in System Settings on macOS, and
//! `LC_TIME`, `LANG` and so on on Linux) unless one is chosen on the Settings
//! tab. These can differ from the system's language: someone can use their
//! computer in English and write dates the German way. The words, such as
//! month names, are in the interface's language (`i18n.rs`), and files
//! always use one format (see `transfer.rs`), so they can be moved between
//! computers.

use crate::money::{self, Numbers, Period, SymbolAt, YearStart};
use chrono::NaiveDate;

/// The order of the day, month and year in a short date.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DateOrder {
    Dmy,
    Mdy,
    Ymd,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Region {
    /// A BCP 47 tag, such as "en-GB". Its name is the message
    /// `region-<tag>`.
    pub tag: &'static str,
    /// The usual currency symbol.
    pub currency: &'static str,
    pub symbol_at: SymbolAt,
    pub numbers: Numbers,
    pub dates: DateOrder,
    /// Between the day, month and year in a short date.
    pub date_sep: char,
}

/// When nothing better is known, as before regions were detected.
pub const DEFAULT_TAG: &str = "en-GB";

const NBSP: char = '\u{a0}';
const DOT: Numbers = Numbers { decimal: '.', group: ',' };
const COMMA_DOT: Numbers = Numbers { decimal: ',', group: '.' };
const COMMA_SPACE: Numbers = Numbers { decimal: ',', group: NBSP };

const fn region(
    tag: &'static str,
    currency: &'static str,
    symbol_at: SymbolAt,
    numbers: Numbers,
    dates: DateOrder,
    date_sep: char,
) -> Region {
    Region { tag, currency, symbol_at, numbers, dates, date_sep }
}

use DateOrder::{Dmy, Mdy, Ymd};
use SymbolAt::{After, Before, BeforeSpaced};

/// Every region that can be chosen, in the order listed. Detection uses the
/// first one for a country or language, so each language's main country
/// comes before its others.
pub const REGIONS: &[Region] = &[
    region("en-GB", "£", Before, DOT, Dmy, '/'),
    region("en-US", "$", Before, DOT, Mdy, '/'),
    region("en-AU", "$", Before, DOT, Dmy, '/'),
    region("en-CA", "$", Before, DOT, Ymd, '-'),
    region("en-IE", "€", Before, DOT, Dmy, '/'),
    region("en-IN", "₹", Before, DOT, Dmy, '/'),
    region("en-NZ", "$", Before, DOT, Dmy, '/'),
    region("en-SG", "$", Before, DOT, Dmy, '/'),
    region("en-ZA", "R", Before, COMMA_SPACE, Ymd, '/'),
    region("de-DE", "€", After, COMMA_DOT, Dmy, '.'),
    region("de-AT", "€", BeforeSpaced, COMMA_SPACE, Dmy, '.'),
    region("de-CH", "CHF", BeforeSpaced, Numbers { decimal: '.', group: '’' }, Dmy, '.'),
    region("fr-FR", "€", After, COMMA_SPACE, Dmy, '/'),
    region("fr-CA", "$", After, COMMA_SPACE, Ymd, '-'),
    region("fr-BE", "€", After, COMMA_SPACE, Dmy, '/'),
    region("nl-NL", "€", BeforeSpaced, COMMA_DOT, Dmy, '-'),
    region("nl-BE", "€", BeforeSpaced, COMMA_DOT, Dmy, '/'),
    region("es-ES", "€", After, COMMA_DOT, Dmy, '/'),
    region("es-MX", "$", Before, DOT, Dmy, '/'),
    region("es-AR", "$", BeforeSpaced, COMMA_DOT, Dmy, '/'),
    region("it-IT", "€", After, COMMA_DOT, Dmy, '/'),
    region("pt-PT", "€", After, COMMA_SPACE, Dmy, '/'),
    region("pt-BR", "R$", BeforeSpaced, COMMA_DOT, Dmy, '/'),
    region("da-DK", "kr.", After, COMMA_DOT, Dmy, '.'),
    region("fi-FI", "€", After, COMMA_SPACE, Dmy, '.'),
    region("nb-NO", "kr", BeforeSpaced, COMMA_SPACE, Dmy, '.'),
    region("sv-SE", "kr", After, COMMA_SPACE, Ymd, '-'),
    region("pl-PL", "zł", After, COMMA_SPACE, Dmy, '.'),
    region("cs-CZ", "Kč", After, COMMA_SPACE, Dmy, '.'),
    region("hu-HU", "Ft", After, COMMA_SPACE, Ymd, '.'),
    region("el-GR", "€", After, COMMA_DOT, Dmy, '/'),
    region("tr-TR", "₺", Before, COMMA_DOT, Dmy, '.'),
    region("ru-RU", "₽", After, COMMA_SPACE, Dmy, '.'),
    region("uk-UA", "₴", After, COMMA_SPACE, Dmy, '.'),
    region("ja-JP", "¥", Before, DOT, Ymd, '/'),
    region("zh-CN", "¥", Before, DOT, Ymd, '/'),
    region("ko-KR", "₩", Before, DOT, Ymd, '.'),
];

/// The date used in examples, such as "Such as 30/10/2026".
fn example_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 30).expect("a real date")
}

/// The language and country in a tag such as "en-GB", "de_AT.UTF-8" or
/// "zh-Hans-CN", in lower and upper case.
fn split_tag(tag: &str) -> (String, Option<String>) {
    let overridden = region_override(tag);
    let tag = tag.split(['.', '@']).next().unwrap_or_default();
    let mut parts = tag.split(['-', '_']);
    let language = parts.next().unwrap_or_default().to_ascii_lowercase();
    let country = parts
        .find(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic()))
        .map(|p| p.to_ascii_uppercase());
    (language, overridden.or(country))
}

/// The country in a region override, as in "en_US@rg=gbzzzz" or
/// "en-US-u-rg-gbzzzz": someone in the United States whose dates and
/// amounts are written the British way.
fn region_override(tag: &str) -> Option<String> {
    let tag = tag.to_ascii_lowercase();
    let value = ["rg=", "-rg-"].iter().find_map(|key| tag.split_once(key)).map(|(_, value)| value)?;
    let country = value.get(..2).filter(|c| c.chars().all(|c| c.is_ascii_alphabetic()))?;
    (value.get(2..6) == Some("zzzz")).then(|| country.to_ascii_uppercase())
}

/// The listed region with this exact tag.
pub fn find(tag: &str) -> Option<Region> {
    REGIONS.iter().find(|r| r.tag.eq_ignore_ascii_case(tag)).copied()
}

/// The best region for a locale tag: the same language and country if
/// listed, otherwise the country's way of writing things (an English
/// speaker in Germany still writes "1.234,56 €"), otherwise the language's,
/// without a currency symbol, which would only be a guess.
pub fn for_tag(tag: &str) -> Region {
    let (language, country) = split_tag(tag);
    let language_of = |r: &Region| split_tag(r.tag).0;
    let country_of = |r: &Region| split_tag(r.tag).1;
    let exact = REGIONS.iter().find(|r| language_of(r) == language && country_of(r) == country);
    let by_country = || REGIONS.iter().find(|r| country.is_some() && country_of(r) == country);
    if let Some(found) = exact.or_else(by_country) {
        return *found;
    }
    match REGIONS.iter().find(|r| language_of(r) == language) {
        Some(found) if country.is_some() => Region { currency: "", ..*found },
        Some(found) => *found,
        None => find(DEFAULT_TAG).expect("the default region is listed"),
    }
}

/// Whether a currency symbol typed in Settings can be used: no symbol at
/// all, or one with no digits and something other than number marks, so it
/// can't be mistaken for part of an amount. "." and "1$" can't be used.
pub fn is_usable_symbol(symbol: &str) -> bool {
    let is_mark = |c: char| c.is_whitespace() || ".,-+'’".contains(c);
    symbol.is_empty() || (!symbol.chars().any(|c| c.is_ascii_digit()) && !symbol.chars().all(is_mark))
}

/// Removes a currency symbol from the start or end of a typed amount, after
/// a leading minus sign if there is one. A symbol that can't be used, such
/// as "." or "-", is left alone rather than changing the number.
fn strip_symbol(text: &str, symbol: &str) -> String {
    if symbol.is_empty() || !is_usable_symbol(symbol) {
        return text.to_owned();
    }
    let (sign, rest) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest.trim_start()),
        None => ("", text),
    };
    let rest = rest.strip_prefix(symbol).unwrap_or(rest).trim();
    let rest = rest.strip_suffix(symbol).unwrap_or(rest).trim_end();
    format!("{sign}{rest}")
}

/// The system's region setting as a locale tag, such as "en-GB", or
/// "en_DE" for English in Germany. Where the region setting can't be read,
/// the system's language stands in for it.
pub fn system_tag() -> String {
    system_region()
        .filter(|tag| is_set(tag))
        .or_else(sys_locale::get_locale)
        .unwrap_or_else(|| DEFAULT_TAG.to_owned())
}

/// Whether a locale tag names a place, rather than being empty or the
/// plain "C" or "POSIX" locale.
fn is_set(tag: &str) -> bool {
    let tag = tag.split(['.', '@']).next().unwrap_or_default();
    !tag.is_empty() && tag != "C" && tag != "POSIX"
}

/// The regional format chosen in Settings, under Time & language.
#[cfg(windows)]
fn system_region() -> Option<String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultLocaleName(name: *mut u16, length: i32) -> i32;
    }
    // LOCALE_NAME_MAX_LENGTH, which includes the closing null.
    let mut name = [0u16; 85];
    // SAFETY: the buffer is passed with its length, and the result is the
    // number of characters written, including the null, or 0 on failure.
    let written = unsafe { GetUserDefaultLocaleName(name.as_mut_ptr(), name.len() as i32) };
    let written = usize::try_from(written).ok().filter(|n| (2..=name.len()).contains(n))?;
    Some(String::from_utf16_lossy(&name[..written - 1]))
}

/// The Region chosen in System Settings, under General, Language & Region,
/// as in "en_DE".
#[cfg(target_os = "macos")]
fn system_region() -> Option<String> {
    use std::ffi::{CStr, c_char, c_void};
    type CFTypeRef = *const c_void;
    const UTF8: u32 = 0x0800_0100;
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFLocaleCopyCurrent() -> CFTypeRef;
        fn CFLocaleGetIdentifier(locale: CFTypeRef) -> CFTypeRef;
        fn CFStringGetCString(string: CFTypeRef, buffer: *mut c_char, size: isize, encoding: u32) -> u8;
        fn CFRelease(object: CFTypeRef);
    }
    // SAFETY: the locale is released once, after its identifier, which it
    // owns, has been copied out. The buffer is passed with its size, and
    // holds a null-terminated string when CFStringGetCString succeeds.
    unsafe {
        let locale = CFLocaleCopyCurrent();
        if locale.is_null() {
            return None;
        }
        let identifier = CFLocaleGetIdentifier(locale);
        let mut buffer = [0 as c_char; 128];
        let copied = !identifier.is_null()
            && CFStringGetCString(identifier, buffer.as_mut_ptr(), buffer.len() as isize, UTF8) != 0;
        let tag = copied.then(|| CStr::from_ptr(buffer.as_ptr()).to_string_lossy().into_owned());
        CFRelease(locale);
        tag
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn system_region() -> Option<String> {
    region_from_env(|name| std::env::var(name).ok())
}

/// The locale for dates and amounts from the environment, as on Linux:
/// `LC_ALL` overrides everything, then the settings for dates, money and
/// numbers, then `LANG`. `LANGUAGE` only affects the language.
#[cfg(any(test, not(any(windows, target_os = "macos"))))]
fn region_from_env(var: impl Fn(&str) -> Option<String>) -> Option<String> {
    ["LC_ALL", "LC_TIME", "LC_MONETARY", "LC_NUMERIC", "LANG"].into_iter().filter_map(var).find(|tag| is_set(tag))
}

/// The region chosen on the Settings tab, or the system's.
pub fn resolve(chosen: Option<&str>) -> Region {
    chosen.and_then(find).unwrap_or_else(|| for_tag(&system_tag()))
}

impl Region {
    /// As listed on the Settings tab, such as "United Kingdom".
    pub fn name(&self) -> String {
        crate::i18n::message(&format!("region-{}", self.tag), None)
    }

    /// An amount as shown, such as "£1,234.56" or "1.234,56 €".
    pub fn money(&self, pence: i64, symbol: &str) -> String {
        money::format(pence, symbol, self.numbers, self.symbol_at)
    }

    /// Reads an amount as typed, such as "12.50" or "12,50 kr".
    pub fn parse_amount(&self, text: &str, symbol: &str) -> Result<i64, String> {
        let mut text = text.trim().to_owned();
        for symbol in [symbol, self.currency] {
            // Symbols such as "kr." end with a dot that would read as a decimal mark.
            text = strip_symbol(&text, symbol);
        }
        money::parse(&text, self.numbers)
    }

    /// An amount as put in a box to be changed, such as "1234.56".
    pub fn editable_amount(&self, pence: i64) -> String {
        money::editable(pence, self.numbers)
    }

    /// An example amount, such as "12.50" or "12,50".
    pub fn example_amount(&self) -> String {
        self.editable_amount(1250)
    }

    /// A date as typed, such as "30/10/2026", "10/30/2026" or "2026-10-30".
    pub fn short_date(&self, date: NaiveDate) -> String {
        let s = self.date_sep;
        match self.dates {
            Dmy => format!("{}{s}{}{s}{}", date.format("%d"), date.format("%m"), date.format("%Y")),
            Mdy => format!("{}{s}{}{s}{}", date.format("%m"), date.format("%d"), date.format("%Y")),
            Ymd => format!("{}{s}{}{s}{}", date.format("%Y"), date.format("%m"), date.format("%d")),
        }
    }

    /// How a short date is written, such as "day/month/year".
    pub fn date_pattern(&self) -> String {
        let s = self.date_sep;
        match self.dates {
            Dmy => format!("{}{s}{}{s}{}", t!("date-day"), t!("date-month"), t!("date-year")),
            Mdy => format!("{}{s}{}{s}{}", t!("date-month"), t!("date-day"), t!("date-year")),
            Ymd => format!("{}{s}{}{s}{}", t!("date-year"), t!("date-month"), t!("date-day")),
        }
    }

    /// An example date, such as "30/10/2026".
    pub fn example_date(&self) -> String {
        self.short_date(example_date())
    }

    /// Reads a date as typed. The day, month and year can be separated by
    /// "/", ".", "-" or spaces, and the year can be two digits. A date
    /// written year-month-day, as in files, always works too.
    pub fn parse_date(&self, text: &str) -> Result<NaiveDate, String> {
        let trimmed = text.trim().trim_end_matches('.');
        let error = || t!("date-invalid", text = text.trim(), pattern = self.date_pattern(), example = self.example_date());
        let parts: Vec<&str> = trimmed.split(['/', '.', '-', ' ']).filter(|p| !p.is_empty()).collect();
        let [a, b, c] = parts[..] else { return Err(error()) };
        let (year, month, day) = match self.dates {
            _ if a.len() == 4 => (a, b, c),
            Dmy => (c, b, a),
            Mdy => (c, a, b),
            Ymd => (a, b, c),
        };
        let number = |s: &str| s.parse::<u32>().ok().filter(|_| s.len() <= 2);
        let year = match (year.len(), year.parse::<i32>()) {
            (2, Ok(y)) => 2000 + y,
            (4, Ok(y)) => y,
            _ => return Err(error()),
        };
        let (Some(month), Some(day)) = (number(month), number(day)) else { return Err(error()) };
        NaiveDate::from_ymd_opt(year, month, day).ok_or_else(error)
    }

    /// When the personal tax year usually starts: 6 April in the United
    /// Kingdom, and the calendar year where that is the tax year.
    pub fn year_start(&self) -> YearStart {
        let (month, day) = match self.tag {
            "en-GB" => (4, 6),
            "en-AU" => (7, 1),
            "en-IN" | "en-NZ" => (4, 1),
            "en-ZA" => (3, 1),
            _ => (1, 1),
        };
        YearStart { month, day }
    }

    /// A day of the year, such as "6 April" or "April 6".
    pub fn day_and_month(&self, start: YearStart) -> String {
        let month = crate::i18n::message(&format!("month-{}", start.month), None);
        let day = start.day.to_string();
        match self.dates {
            Dmy => t!("day-month-dm", day = day, month = month),
            Mdy | Ymd => t!("day-month-md", day = day, month = month),
        }
    }

    /// A date without its weekday, such as "6 Apr 2026" or "Apr 6, 2026".
    pub fn medium_date(&self, date: NaiveDate) -> String {
        use chrono::Datelike;
        let month = money::short_month_name(date.month());
        let (day, year) = (date.day().to_string(), date.year().to_string());
        match self.dates {
            Dmy => t!("medium-date-dmy", day = day, month = month, year = year),
            Mdy => t!("medium-date-mdy", day = day, month = month, year = year),
            Ymd => t!("medium-date-ymd", day = day, month = month, year = year),
        }
    }

    /// The dates shown, such as "October 2026" or "6 Apr 2026 to 5 Apr 2027".
    pub fn period_label(&self, period: Period) -> String {
        match period {
            Period::Month(m) => m.label(),
            Period::Days { first, last } => t!("period-dates", first = self.medium_date(first), last = self.medium_date(last)),
        }
    }

    /// The dates shown at the start of a sentence, such as "In October 2026"
    /// or "From 6 Apr 2026 to 5 Apr 2027".
    pub fn period_opening(&self, period: Period) -> String {
        match period {
            Period::Month(m) => t!("period-in-month", month = m.label()),
            Period::Days { first, last } => {
                t!("period-from-to", first = self.medium_date(first), last = self.medium_date(last))
            }
        }
    }

    /// A date as said and shown in lists, such as "Fri 30 Oct 2026" or
    /// "Fri, Oct 30, 2026". The region decides the order of the day, month
    /// and year, and the language the words and punctuation.
    pub fn long_date(&self, date: NaiveDate) -> String {
        use chrono::Datelike;
        let weekday = crate::i18n::message(&format!("weekday-short-{}", date.weekday().number_from_monday()), None);
        let month = money::short_month_name(date.month());
        let (day, year) = (date.day().to_string(), date.year().to_string());
        match self.dates {
            Dmy => t!("long-date-dmy", weekday = weekday, day = day, month = month, year = year),
            Mdy => t!("long-date-mdy", weekday = weekday, day = day, month = month, year = year),
            Ymd => t!("long-date-ymd", weekday = weekday, day = day, month = month, year = year),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn finds_regions_from_system_tags() {
        assert_eq!(for_tag("en-GB").tag, "en-GB");
        assert_eq!(for_tag("en_US.UTF-8").tag, "en-US");
        assert_eq!(for_tag("de_AT@euro").tag, "de-AT");
        assert_eq!(for_tag("zh-Hans-CN").tag, "zh-CN");
        assert_eq!(for_tag("en-DE").tag, "de-DE", "the country's way of writing");
        assert_eq!(for_tag("fr").tag, "fr-FR", "the language's main country");
        let nigeria = for_tag("en-NG");
        assert_eq!((nigeria.numbers, nigeria.currency), (DOT, ""), "no guessed currency");
        assert_eq!(for_tag("C").tag, DEFAULT_TAG);
        assert_eq!(for_tag("").tag, DEFAULT_TAG);
        assert!(REGIONS.iter().all(|r| for_tag(r.tag) == *r), "every region can be detected");
        assert_eq!(for_tag("en_DE").tag, "de-DE", "macOS: English, with the region set to Germany");
        assert_eq!(for_tag("en_US@rg=gbzzzz").tag, "en-GB", "a region override");
        assert_eq!(for_tag("en-US-u-rg-dezzzz").tag, "de-DE");
        assert_eq!(for_tag("en_US@rg=1").tag, "en-US", "a broken override is ignored");
    }

    #[test]
    fn region_from_linux_environment() {
        fn region(vars: &[(&str, &str)]) -> Option<String> {
            region_from_env(|name| vars.iter().find(|(n, _)| *n == name).map(|(_, v)| v.to_string()))
        }
        // LANGUAGE is the language only; LC_TIME says how dates are written.
        let vars = [("LANGUAGE", "en_GB"), ("LANG", "en_GB.UTF-8"), ("LC_TIME", "de_DE.UTF-8")];
        assert_eq!(region(&vars).as_deref(), Some("de_DE.UTF-8"));
        let vars = [("LC_ALL", "fr_FR.UTF-8"), ("LC_TIME", "de_DE.UTF-8")];
        assert_eq!(region(&vars).as_deref(), Some("fr_FR.UTF-8"), "LC_ALL overrides");
        let vars = [("LC_ALL", ""), ("LC_TIME", "C"), ("LC_MONETARY", "POSIX"), ("LANG", "sv_SE.UTF-8")];
        assert_eq!(region(&vars).as_deref(), Some("sv_SE.UTF-8"), "empty and C are skipped");
        assert_eq!(region(&[("LANG", "C.UTF-8")]), None);
    }

    /// The real setting, on the systems with their own way of reading it.
    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn reads_the_systems_region() {
        let tag = system_region().expect("the region setting can be read");
        assert!(is_set(&tag), "{tag:?}");
        assert!(split_tag(&tag).1.is_some(), "{tag:?} names a country");
    }

    #[test]
    fn writes_and_reads_dates() {
        let gb = find("en-GB").unwrap();
        let us = find("en-US").unwrap();
        let de = find("de-DE").unwrap();
        let se = find("sv-SE").unwrap();
        let d = date(2026, 3, 5);
        assert_eq!(gb.short_date(d), "05/03/2026");
        assert_eq!(us.short_date(d), "03/05/2026");
        assert_eq!(de.short_date(d), "05.03.2026");
        assert_eq!(se.short_date(d), "2026-03-05");
        assert_eq!(gb.long_date(d), "Thu 5 Mar 2026");
        assert_eq!(us.long_date(d), "Thu, Mar 5, 2026");
        assert_eq!(gb.medium_date(d), "5 Mar 2026");
        assert_eq!(us.medium_date(d), "Mar 5, 2026");
        assert_eq!(gb.day_and_month(gb.year_start()), "6 April");
        assert_eq!(us.day_and_month(us.year_start()), "January 1");
        let year = gb.year_start().year_of(d);
        assert_eq!(gb.period_label(year), "6 Apr 2025 to 5 Apr 2026");
        assert_eq!(gb.period_opening(year), "From 6 Apr 2025 to 5 Apr 2026");
        for region in REGIONS {
            assert_eq!(region.parse_date(&region.short_date(d)), Ok(d), "{}", region.tag);
            assert_eq!(region.parse_date("2026-03-05"), Ok(d), "{} reads file dates", region.tag);
        }
        assert_eq!(gb.parse_date("5/3/26"), Ok(d));
        assert_eq!(us.parse_date("3-5-2026"), Ok(d));
        assert_eq!(de.parse_date(" 5. 3. 2026 "), Ok(d));
        assert_eq!(find("hu-HU").unwrap().parse_date("2026. 03. 05."), Ok(d));
        assert_eq!(
            gb.parse_date("30/02/2026"),
            Err("\"30/02/2026\" is not a date. Type one as day/month/year, such as 30/10/2026.".into())
        );
        assert!(us.parse_date("30/10/2026").is_err());
        assert!(gb.parse_date("5/3").is_err());
        assert!(gb.parse_date("5/3/202").is_err());
    }

    #[test]
    fn writes_and_reads_amounts() {
        let de = find("de-DE").unwrap();
        assert_eq!(de.money(123_456, de.currency), "1.234,56\u{a0}€");
        assert_eq!(de.parse_amount("8,50 €", "€"), Ok(850));
        let dk = find("da-DK").unwrap();
        assert_eq!(dk.money(123_456, dk.currency), "1.234,56\u{a0}kr.");
        assert_eq!(dk.parse_amount(&dk.money(123_456, dk.currency), dk.currency), Ok(123_456));
        let ch = find("de-CH").unwrap();
        assert_eq!(ch.money(123_456, ch.currency), "CHF\u{a0}1’234.56");
        assert_eq!(ch.parse_amount("CHF 1’234.56", ch.currency), Ok(123_456));
        let gb = find("en-GB").unwrap();
        assert_eq!(gb.parse_amount("-£3", "£"), Ok(-300));
        assert_eq!(gb.parse_amount("£-3", "£"), Ok(-300));
        // A symbol made of number marks is not taken out of the amount.
        assert_eq!(gb.parse_amount("8.50", "."), Ok(850));
        assert_eq!(gb.parse_amount("-3", "-"), Ok(-300));
        assert_eq!(gb.parse_amount("15", "1$"), Ok(1500));
        for symbol in ["", "£", "kr.", "CHF", "R$", "zł"] {
            assert!(is_usable_symbol(symbol), "{symbol:?}");
        }
        for symbol in [".", "-", ", ", "1$", "0"] {
            assert!(!is_usable_symbol(symbol), "{symbol:?}");
        }
        assert_eq!(find("en-GB").unwrap().example_amount(), "12.50");
        assert_eq!(de.example_amount(), "12,50");
    }
}
