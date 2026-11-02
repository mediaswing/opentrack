//! Locale-aware display of money and dates.
//!
//! The system locale (e.g. `en-GB`, `de_DE.UTF-8`) is read with `sys-locale` and
//! mapped through small built-in tables to a currency symbol, number separators,
//! date order and month names. Unknown locales fall back sensibly field by field.

use jiff::civil::Date;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateOrder {
    Dmy,
    Mdy,
    Ymd,
}

#[derive(Debug, Clone)]
pub struct Locale {
    /// Normalised tag, e.g. `en-GB`.
    pub tag: String,
    pub currency_symbol: String,
    /// `12,50 €` rather than `€12.50`.
    pub symbol_after: bool,
    /// Space between symbol and number.
    pub symbol_space: bool,
    pub decimal: char,
    pub group: char,
    pub date_order: DateOrder,
    pub date_sep: char,
    months: [&'static str; 12],
}

const EUROZONE: &[&str] = &[
    "AT", "BE", "CY", "DE", "EE", "ES", "FI", "FR", "GR", "HR", "IE", "IT", "LT", "LU", "LV", "MT", "NL", "PT", "SI",
    "SK",
];

/// Region used when the locale has a language but no region (e.g. plain `de`).
fn default_region(lang: &str) -> &'static str {
    match lang {
        "en" => "GB",
        "de" => "DE",
        "fr" => "FR",
        "es" => "ES",
        "it" => "IT",
        "nl" => "NL",
        "pt" => "PT",
        "sv" => "SE",
        "nb" | "nn" | "no" => "NO",
        "da" => "DK",
        "fi" => "FI",
        "pl" => "PL",
        "cs" => "CZ",
        "hu" => "HU",
        "ja" => "JP",
        "zh" => "CN",
        "ko" => "KR",
        "ru" => "RU",
        "tr" => "TR",
        "el" => "GR",
        "ga" => "IE",
        _ => "",
    }
}

/// `(symbol, symbol_after, symbol_space)` for a region; language matters inside the eurozone and Canada.
fn currency(lang: &str, region: &str) -> (&'static str, bool, bool) {
    if EUROZONE.contains(&region) {
        return match lang {
            "en" | "ga" | "mt" => ("€", false, false),
            "nl" => ("€", false, true),
            _ => ("€", true, true),
        };
    }
    match region {
        "GB" | "GG" | "JE" | "IM" | "GI" => ("£", false, false),
        "US" | "AU" | "NZ" | "SG" | "HK" | "MX" => ("$", false, false),
        "CA" if lang == "fr" => ("$", true, true),
        "CA" => ("$", false, false),
        "CH" | "LI" => ("CHF", false, true),
        "SE" | "NO" | "DK" | "IS" => ("kr", true, true),
        "PL" => ("zł", true, true),
        "CZ" => ("Kč", true, true),
        "HU" => ("Ft", true, true),
        "RO" => ("lei", true, true),
        "BG" => ("лв.", true, true),
        "RU" => ("₽", true, true),
        "UA" => ("₴", true, true),
        "TR" => ("₺", false, false),
        "IN" => ("₹", false, false),
        "JP" => ("¥", false, false),
        "CN" => ("¥", false, false),
        "KR" => ("₩", false, false),
        "BR" => ("R$", false, true),
        "ZA" => ("R", false, false),
        "IL" => ("₪", true, true),
        "TW" => ("NT$", false, false),
        "AR" | "CL" | "CO" | "UY" => ("$", false, true),
        "PH" => ("₱", false, false),
        "TH" => ("฿", false, false),
        "VN" => ("₫", true, true),
        "ID" => ("Rp", false, false),
        "MY" => ("RM", false, false),
        "NG" => ("₦", false, false),
        "AE" => ("AED", false, true),
        "SA" => ("SAR", false, true),
        "EG" => ("E£", false, false),
        "PK" => ("Rs", false, false),
        // Unknown: the generic currency sign rather than a wrong currency.
        // Settings → Region can replace it.
        _ => (GENERIC_CURRENCY, false, false),
    }
}

/// Shown when the region's currency isn't known.
pub const GENERIC_CURRENCY: &str = "¤";

/// `(decimal, group)` separators.
fn separators(lang: &str, region: &str) -> (char, char) {
    match (lang, region) {
        (_, "CH" | "LI") => ('.', '\''),
        ("es", "MX" | "US") => ('.', ','),
        ("en", "ZA") => (',', ' '),
        ("en", _) | ("ja" | "zh" | "ko" | "he" | "th" | "ga" | "mt", _) => ('.', ','),
        ("de" | "es" | "it" | "nl" | "pt" | "da" | "id" | "tr" | "el" | "ro" | "hr" | "sl", _) => (',', '.'),
        (
            "fr" | "sv" | "nb" | "nn" | "no" | "fi" | "cs" | "sk" | "pl" | "ru" | "uk" | "hu" | "et" | "lv" | "lt"
            | "bg",
            _,
        ) => (',', ' '),
        _ => ('.', ','),
    }
}

fn date_format(lang: &str, region: &str) -> (DateOrder, char) {
    match region {
        "US" => (DateOrder::Mdy, '/'),
        "CA" => (DateOrder::Ymd, '-'),
        "SE" | "LT" => (DateOrder::Ymd, '-'),
        "JP" | "CN" | "TW" | "ZA" => (DateOrder::Ymd, '/'),
        "KR" | "HU" => (DateOrder::Ymd, '.'),
        "NL" => (DateOrder::Dmy, '-'),
        "DE" | "AT" | "CH" | "LI" | "NO" | "DK" | "FI" | "CZ" | "SK" | "PL" | "RU" | "UA" | "TR" | "RO" | "BG"
        | "EE" | "LV" | "HR" | "SI" | "IS" => (DateOrder::Dmy, '.'),
        _ if lang == "de" => (DateOrder::Dmy, '.'),
        _ => (DateOrder::Dmy, '/'),
    }
}

fn months(lang: &str) -> [&'static str; 12] {
    match lang {
        "fr" => ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."],
        "de" => ["Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sept.", "Okt.", "Nov.", "Dez."],
        "es" => ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic"],
        "it" => ["gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic"],
        "nl" => ["jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec"],
        "pt" => ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"],
        "sv" | "nb" | "nn" | "no" | "da" => {
            ["jan", "feb", "mar", "apr", "maj", "jun", "jul", "aug", "sep", "okt", "nov", "dec"]
        }
        _ => ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    }
}

/// Split `en_GB.UTF-8`, `en-Latn-GB`, `zh-Hans-CN@x` etc. into lowercase language and uppercase region.
fn parse_tag(raw: &str) -> (String, String) {
    let base = raw.split(['.', '@']).next().unwrap_or("");
    let mut parts = base.split(['-', '_']).filter(|p| !p.is_empty());
    let lang = parts.next().unwrap_or("").to_ascii_lowercase();
    let region = parts
        .find(|p| {
            (p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic()))
                || (p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))
        })
        .map(|p| p.to_ascii_uppercase())
        .unwrap_or_default();
    (lang, region)
}

impl Locale {
    /// The OS locale, falling back to `en-GB` if it cannot be read.
    pub fn system_tag() -> String {
        sys_locale::get_locale().unwrap_or_else(|| "en-GB".into())
    }

    pub fn from_tag(raw: &str) -> Self {
        let (mut lang, mut region) = parse_tag(raw);
        if lang.is_empty() || lang == "c" || lang == "posix" {
            lang = "en".into();
        }
        if region.is_empty() {
            region = default_region(&lang).to_string();
        }
        let (symbol, symbol_after, symbol_space) = currency(&lang, &region);
        let (decimal, group) = separators(&lang, &region);
        let (date_order, date_sep) = date_format(&lang, &region);
        let tag = if region.is_empty() { lang.clone() } else { format!("{lang}-{region}") };
        Self {
            tag,
            currency_symbol: symbol.into(),
            symbol_after,
            symbol_space,
            decimal,
            group,
            date_order,
            date_sep,
            months: months(&lang),
        }
    }

    /// Plain number with locale separators but no symbol and no grouping, e.g. `1234,50`.
    /// Used to pre-fill editable fields.
    pub fn format_amount(&self, pence: i64) -> String {
        let sign = if pence < 0 { "-" } else { "" };
        let abs = pence.unsigned_abs();
        format!("{sign}{}{}{:02}", abs / 100, self.decimal, abs % 100)
    }

    /// Full display form, e.g. `£1,234.50`, `1.234,50 €`, `-CHF 12.00`.
    pub fn format_money(&self, pence: i64) -> String {
        let sign = if pence < 0 { "-" } else { "" };
        let abs = pence.unsigned_abs();
        let whole = (abs / 100).to_string();
        let mut number = String::new();
        for (i, ch) in whole.chars().enumerate() {
            if i > 0 && (whole.len() - i).is_multiple_of(3) {
                number.push(self.group);
            }
            number.push(ch);
        }
        number.push(self.decimal);
        number.push_str(&format!("{:02}", abs % 100));
        let space = if self.symbol_space { " " } else { "" };
        let sym = &self.currency_symbol;
        if self.symbol_after { format!("{sign}{number}{space}{sym}") } else { format!("{sign}{sym}{space}{number}") }
    }

    pub fn month_short(&self, month: i8) -> &'static str {
        self.months[(month.clamp(1, 12) - 1) as usize]
    }

    /// Three-letter month abbreviation, e.g. `Nov`, `Okt`, `Mär`.
    pub fn month_abbr3(&self, month: i8) -> String {
        self.month_short(month).chars().filter(|c| c.is_alphabetic()).take(3).collect()
    }

    /// e.g. `Nov 2026`.
    pub fn month_label(&self, (y, m): (i16, i8)) -> String {
        format!("{} {y}", self.month_short(m))
    }

    /// strftime pattern for a full numeric date, e.g. `%d/%m/%Y`.
    pub fn date_pattern(&self) -> String {
        let s = self.date_sep;
        match self.date_order {
            DateOrder::Dmy => format!("%d{s}%m{s}%Y"),
            DateOrder::Mdy => format!("%m{s}%d{s}%Y"),
            DateOrder::Ymd => format!("%Y{s}%m{s}%d"),
        }
    }

    /// Full numeric date, e.g. `02/11/2026`, `11/02/2026`, `2026-11-02`.
    pub fn format_date(&self, d: Date) -> String {
        d.strftime(&self.date_pattern()).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d() -> Date {
        Date::new(2026, 11, 2).unwrap()
    }

    #[test]
    fn uk() {
        let l = Locale::from_tag("en-GB");
        assert_eq!(l.format_money(123450), "£1,234.50");
        assert_eq!(l.format_money(-5), "-£0.05");
        assert_eq!(l.format_date(d()), "02/11/2026");
    }

    #[test]
    fn us() {
        let l = Locale::from_tag("en_US.UTF-8");
        assert_eq!(l.format_money(123450), "$1,234.50");
        assert_eq!(l.format_date(d()), "11/02/2026");
    }

    #[test]
    fn germany_and_france() {
        let de = Locale::from_tag("de-DE");
        assert_eq!(de.format_money(123450), "1.234,50 €");
        assert_eq!(de.format_date(d()), "02.11.2026");
        assert_eq!(de.format_amount(1250), "12,50");
        assert_eq!(de.month_label((2026, 3)), "März 2026");
        assert_eq!(de.month_abbr3(9), "Sep");
        assert_eq!(Locale::from_tag("fr-FR").month_abbr3(1), "jan");
        let fr = Locale::from_tag("fr_FR");
        assert_eq!(fr.format_money(123450), "1 234,50 €");
        assert_eq!(Locale::from_tag("en-IE").format_money(100), "€1.00");
    }

    #[test]
    fn odd_tags() {
        assert_eq!(Locale::from_tag("zh-Hans-CN").tag, "zh-CN");
        assert_eq!(Locale::from_tag("de").tag, "de-DE");
        assert_eq!(Locale::from_tag("C").tag, "en-GB");
        assert_eq!(Locale::from_tag("sv-SE").format_date(d()), "2026-11-02");
        assert_eq!(Locale::from_tag("de-CH").format_money(123450), "CHF 1'234.50");
    }

    #[test]
    fn unknown_currency_is_generic() {
        assert_eq!(Locale::from_tag("zh-TW").format_money(123456), "NT$1,234.56");
        assert_eq!(Locale::from_tag("es-AR").currency_symbol, "$");
        assert_eq!(Locale::from_tag("es-419").currency_symbol, GENERIC_CURRENCY);
        assert_eq!(Locale::from_tag("xx-QQ").currency_symbol, GENERIC_CURRENCY);
        assert_eq!(Locale::from_tag("sw").currency_symbol, GENERIC_CURRENCY);
    }
}
