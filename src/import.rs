//! Reading transactions from a CSV file, such as a bank statement export.
//!
//! Nothing here touches the database or the window. [`Table::read`] loads the
//! file, [`Mapping::guess`] works out which column holds what, [`Mapping::rows`]
//! reads each line, and [`plan`] matches the result against what is already
//! recorded, ready for [`crate::db::Store::import`].

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use jiff::civil::Date;

use crate::db::{Category, ImportRow, Spending, validate_category, validate_spending};
use crate::locale::DateOrder;
use crate::money;

/// Larger files are refused: a statement is far smaller, and the whole file
/// is held in memory.
pub const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;

/// Separators tried when working out how the cells are split.
const DELIMITERS: [u8; 4] = *b",;\t|";

/// A CSV file's cells.
#[derive(Debug, Clone)]
pub struct Table {
    /// Each non-blank record, with the line it starts on (from 1).
    pub records: Vec<(u64, Vec<String>)>,
    pub delimiter: u8,
    /// The file wasn't UTF-8, so it was read as Windows-1252 (as Excel saves
    /// "CSV" on Windows).
    pub legacy_encoding: bool,
}

impl Table {
    pub fn read(path: &Path) -> Result<Self> {
        let size = std::fs::metadata(path).with_context(|| format!("reading {}", path.display()))?.len();
        anyhow::ensure!(
            size <= MAX_FILE_BYTES,
            "{} is too large to import ({} MB; at most {} MB)",
            path.display(),
            size / (1024 * 1024),
            MAX_FILE_BYTES / (1024 * 1024)
        );
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let (text, legacy_encoding) = decode(bytes);
        let mut table = Self::parse(&text).with_context(|| format!("reading {}", path.display()))?;
        table.legacy_encoding = legacy_encoding;
        Ok(table)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let delimiter = detect_delimiter(text);
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .delimiter(delimiter)
            .from_reader(text.as_bytes());
        let mut records = Vec::new();
        // The reader's own line count is off with CRLF endings and blank
        // lines, but its byte offsets are right, so count newlines up to them.
        let (mut line, mut counted) = (1, 0);
        for record in reader.records() {
            let record = record.context("the file is not valid CSV")?;
            // After a CRLF the offset is on its `\n`, and skipped blank lines
            // come before the record, so move on to the record's first byte.
            let mut start = record.position().map_or(counted, |p| p.byte() as usize).clamp(counted, text.len());
            while matches!(text.as_bytes().get(start), Some(b'\r' | b'\n')) {
                start += 1;
            }
            line += text.as_bytes()[counted..start].iter().filter(|b| **b == b'\n').count() as u64;
            counted = start;
            let cells: Vec<String> = record.iter().map(|c| c.trim().to_string()).collect();
            if cells.iter().any(|c| !c.is_empty()) {
                records.push((line, cells));
            }
        }
        anyhow::ensure!(!records.is_empty(), "the file has no rows");
        Ok(Self { records, delimiter, legacy_encoding: false })
    }

    /// The most cells on any line.
    pub fn columns(&self) -> usize {
        self.records.iter().map(|(_, r)| r.len()).max().unwrap_or(0)
    }

    /// How the delimiter is shown to the user.
    pub fn delimiter_name(&self) -> &'static str {
        match self.delimiter {
            b';' => "semicolons",
            b'\t' => "tabs",
            b'|' => "vertical bars",
            _ => "commas",
        }
    }
}

/// UTF-8 if it is valid, otherwise Windows-1252. Returns whether it fell back.
fn decode(bytes: Vec<u8>) -> (String, bool) {
    match String::from_utf8(bytes) {
        Ok(text) => (text, false),
        Err(e) => (e.into_bytes().iter().map(|&b| windows_1252(b)).collect(), true),
    }
}

fn windows_1252(b: u8) -> char {
    // 0x80–0x9F differ from Latin-1; the rest map straight to Unicode.
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘',
        '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9f => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

/// The separator that splits the first lines into the same number of cells.
/// Files from places that write `12,50` usually use `;`.
fn detect_delimiter(text: &str) -> u8 {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(20).collect();
    let count = |line: &str, d: u8| {
        let mut quoted = false;
        line.bytes()
            .filter(|&b| {
                if b == b'"' {
                    quoted = !quoted;
                }
                b == d && !quoted
            })
            .count()
    };
    let mut best = (b',', 0, 0);
    for d in DELIMITERS {
        let Some(first) = lines.first().map(|l| count(l, d)).filter(|n| *n > 0) else { continue };
        let consistent = lines.iter().filter(|l| count(l, d) == first).count();
        if (consistent, first) > (best.1, best.2) {
            best = (d, consistent, first);
        }
    }
    best.0
}

/// Which column holds what, and how to read it.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapping {
    /// The first record names the columns.
    pub has_header: bool,
    pub date: Option<usize>,
    pub name: Option<usize>,
    pub amount: Option<usize>,
    pub category: Option<usize>,
    /// How to read numeric dates that don't start with the year.
    pub date_order: DateOrder,
    /// Spending is negative in the file, as on most bank statements.
    pub negate: bool,
}

/// Header words that name each column, most specific first.
const DATE_WORDS: &[&str] = &["date", "datum", "fecha", "data", "posted", "day"];
const NAME_WORDS: &[&str] = &[
    "description",
    "name",
    "payee",
    "merchant",
    "details",
    "memo",
    "narrative",
    "beschreibung",
    "libellé",
    "reference",
];
const AMOUNT_WORDS: &[&str] = &["amount", "price", "value", "betrag", "montant", "importe", "total"];
const CATEGORY_WORDS: &[&str] = &["category", "bucket", "kategorie", "catégorie"];

impl Mapping {
    /// A best guess from the header names, or from the cells if there is no header.
    pub fn guess(table: &Table, default_order: DateOrder) -> Self {
        let first = &table.records[0].1;
        let looks_like_data = first.iter().any(|c| parse_date(c, default_order).is_ok() || money::parse(c).is_ok());
        let mut m = Self {
            has_header: !looks_like_data,
            date: None,
            name: None,
            amount: None,
            category: None,
            date_order: default_order,
            negate: false,
        };

        if m.has_header {
            let headers: Vec<String> = first.iter().map(|h| h.to_lowercase()).collect();
            let mut used = Vec::new();
            let mut find = |words: &[&str]| {
                let found = words
                    .iter()
                    .find_map(|w| headers.iter().enumerate().position(|(i, h)| !used.contains(&i) && h.contains(w)));
                used.extend(found);
                found
            };
            m.date = find(DATE_WORDS);
            m.amount = find(AMOUNT_WORDS);
            m.category = find(CATEGORY_WORDS);
            m.name = find(NAME_WORDS);
        }

        // Fill the gaps from what the cells look like.
        let sample: Vec<&Vec<String>> = m.data(table).take(50).map(|(_, r)| r).collect();
        let mostly = |col: usize, test: &dyn Fn(&str) -> bool| {
            let cells: Vec<&str> =
                sample.iter().filter_map(|r| r.get(col)).map(String::as_str).filter(|c| !c.is_empty()).collect();
            !cells.is_empty() && cells.iter().filter(|c| test(c)).count() * 5 >= cells.len() * 4
        };
        let is_date = |c: &str| parse_date(c, DateOrder::Dmy).is_ok() || parse_date(c, DateOrder::Mdy).is_ok();
        let is_amount = |c: &str| money::parse(c).is_ok();
        let taken = |m: &Self, i: usize| [m.date, m.name, m.amount, m.category].contains(&Some(i));
        for col in 0..table.columns() {
            if m.date.is_none() && !taken(&m, col) && mostly(col, &is_date) {
                m.date = Some(col);
            }
        }
        for col in 0..table.columns() {
            if m.amount.is_none() && !taken(&m, col) && mostly(col, &is_amount) {
                m.amount = Some(col);
            }
        }
        for col in 0..table.columns() {
            if m.name.is_none() && !taken(&m, col) && mostly(col, &|c| !is_amount(c) && !is_date(c)) {
                m.name = Some(col);
            }
        }

        if let Some(col) = m.date {
            m.date_order = detect_date_order(sample.iter().filter_map(|r| r.get(col)), default_order);
        }
        if let Some(col) = m.amount {
            let amounts: Vec<i64> =
                sample.iter().filter_map(|r| r.get(col)).filter_map(|c| money::parse(c).ok()).collect();
            m.negate = amounts.iter().filter(|p| **p < 0).count() * 2 > amounts.iter().filter(|p| **p != 0).count();
        }
        m
    }

    /// The records after the header, if there is one.
    pub fn data<'t>(&self, table: &'t Table) -> impl Iterator<Item = &'t (u64, Vec<String>)> {
        table.records.iter().skip(usize::from(self.has_header))
    }

    /// Read every data record.
    pub fn rows(&self, table: &Table) -> Vec<Result<Parsed, RowError>> {
        self.data(table).map(|(line, cells)| self.row(*line, cells)).collect()
    }

    fn row(&self, line: u64, cells: &[String]) -> Result<Parsed, RowError> {
        let fail = |message: String| RowError { line, message };
        let cell = |col: Option<usize>| col.and_then(|c| cells.get(c)).map_or("", String::as_str);
        let date = parse_date(cell(self.date), self.date_order).map_err(fail)?;
        let pence = money::parse(cell(self.amount)).map_err(fail)?;
        let pence = if self.negate { -pence } else { pence };
        let name = cell(self.name).to_string();
        validate_spending(&Spending { line: 0, name: name.clone(), date, pence, bucket: 0, owner: None, team: None })
            .map_err(|e| fail(e.to_string()))?;
        let category = Some(cell(self.category).to_string()).filter(|c| !c.is_empty());
        Ok(Parsed { line, date, name, pence, category })
    }
}

/// A record read from the file.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub line: u64,
    pub date: Date,
    pub name: String,
    pub pence: i64,
    pub category: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowError {
    pub line: u64,
    pub message: String,
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const WEEKDAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// Runs of digits and runs of letters, so `2026-11-02T10:00` gives `2026`,
/// `11`, `02`, `T`, `10`, `00`.
fn tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = None;
    let kind = |c: char| if c.is_ascii_digit() { 1 } else if c.is_alphabetic() { 2 } else { 0 };
    let mut last = 0;
    for (i, c) in text.char_indices() {
        let k = kind(c);
        if k != last {
            if let Some(s) = start.take() {
                out.push(&text[s..i]);
            }
            if k != 0 {
                start = Some(i);
            }
        }
        last = k;
    }
    if let Some(s) = start {
        out.push(&text[s..]);
    }
    out
}

fn month_from_name(word: &str) -> Option<i8> {
    let lower = word.to_lowercase();
    let abbr = lower.get(..3)?;
    MONTHS.iter().position(|m| *m == abbr).map(|i| i as i8 + 1)
}

/// Read a date such as `2026-11-02`, `02/11/2026`, `2.11.26`, `2 Nov 2026`
/// or `Nov 2nd, 2026`. A time after the date is ignored. `order` says how to
/// read numeric dates that don't start with a four-digit year.
pub fn parse_date(text: &str, order: DateOrder) -> Result<Date, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Missing date".into());
    }
    let invalid = || format!("'{text}' is not a date");
    let mut parts: Vec<&str> = tokens(text)
        .into_iter()
        .filter(|t| !matches!(t.to_lowercase().as_str(), "st" | "nd" | "rd" | "th"))
        .collect();
    if parts.first().is_some_and(|t| t.len() >= 3 && WEEKDAYS.iter().any(|w| t.to_lowercase().starts_with(w))) {
        parts.remove(0);
    }
    let [a, b, c] = parts.get(..3).ok_or_else(invalid)?.try_into().map_err(|_| invalid())?;
    // `08:01:12` is a time, not a date. A time may follow the date.
    let date_end = c.as_ptr() as usize - text.as_ptr() as usize + c.len();
    if text[..date_end].contains(':') {
        return Err(invalid());
    }
    let num = |t: &str| t.parse::<i16>().ok().filter(|_| t.len() <= 4 && t.bytes().all(|b| b.is_ascii_digit()));

    let (year, month, day) = if let Some(i) = [a, b, c].iter().position(|t| month_from_name(t).is_some()) {
        let month = month_from_name([a, b, c][i]).ok_or_else(invalid)?;
        let rest: Vec<&str> = [a, b, c].into_iter().enumerate().filter(|(j, _)| *j != i).map(|(_, t)| t).collect();
        let (y, d) = if rest[0].len() == 4 { (rest[0], rest[1]) } else { (rest[1], rest[0]) };
        (y, i16::from(month), num(d).ok_or_else(invalid)?)
    } else {
        let (y, m, d) = if a.len() == 4 {
            (a, b, c)
        } else {
            match order {
                DateOrder::Dmy => (c, b, a),
                DateOrder::Mdy => (c, a, b),
                DateOrder::Ymd => (a, b, c),
            }
        };
        (y, num(m).ok_or_else(invalid)?, num(d).ok_or_else(invalid)?)
    };
    let year = match (year.len(), num(year)) {
        (2, Some(y)) => 2000 + y,
        (4, Some(y)) => y,
        _ => return Err(invalid()),
    };
    // Checked before narrowing to `i8`, so a month of 268 can't wrap round to 12.
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(invalid());
    }
    Date::new(year, month as i8, day as i8).map_err(|_| format!("'{text}' is not a real date"))
}

/// Day-first or month-first, from whichever of the first two numbers goes
/// above 12. If none does, the locale's order.
fn detect_date_order<'a>(dates: impl Iterator<Item = &'a String>, default: DateOrder) -> DateOrder {
    for d in dates {
        let parts = tokens(d);
        let [a, b, _, ..] = parts.as_slice() else { continue };
        if a.len() == 4 || !a.bytes().chain(b.bytes()).all(|c| c.is_ascii_digit()) {
            continue;
        }
        match (a.parse::<u32>(), b.parse::<u32>()) {
            (Ok(a), _) if a > 12 => return DateOrder::Dmy,
            (_, Ok(b)) if b > 12 => return DateOrder::Mdy,
            _ => {}
        }
    }
    if default == DateOrder::Ymd { DateOrder::Dmy } else { default }
}

/// What to do with the rows read from the file.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// For rows without a category, and for unknown ones unless they are created.
    pub default_category: Option<i64>,
    /// Add categories named in the file that don't exist yet.
    pub create_categories: bool,
    /// Leave out rows with the same date, description and amount as one
    /// already recorded.
    pub skip_duplicates: bool,
}

/// What an import would do.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub rows: Vec<ImportRow>,
    pub errors: Vec<RowError>,
    /// Rows left out because they are already recorded.
    pub duplicates: usize,
    /// Categories that would be added, as first spelled in the file.
    pub new_categories: Vec<String>,
}

pub fn plan(parsed: Vec<Result<Parsed, RowError>>, categories: &[Category], spending: &[Spending], opts: &Options) -> Plan {
    let key = |date: Date, pence: i64, name: &str| (date, pence, name.trim().to_lowercase());
    let mut recorded: HashMap<_, usize> = HashMap::new();
    if opts.skip_duplicates {
        for s in spending {
            *recorded.entry(key(s.date, s.pence, &s.name)).or_default() += 1;
        }
    }
    let existing: HashMap<String, &str> = categories.iter().map(|c| (c.name.to_lowercase(), c.name.as_str())).collect();
    let default = opts.default_category.and_then(|id| categories.iter().find(|c| c.id == id)).map(|c| c.name.clone());

    let mut plan = Plan::default();
    for row in parsed {
        let row = match row {
            Ok(r) => r,
            Err(e) => {
                plan.errors.push(e);
                continue;
            }
        };
        // Each recorded transaction matches one row, so two identical coffees
        // in the file against one recorded leaves one to import.
        if let Some(n) = recorded.get_mut(&key(row.date, row.pence, &row.name)).filter(|n| **n > 0) {
            *n -= 1;
            plan.duplicates += 1;
            continue;
        }
        let category = match &row.category {
            Some(name) => match existing.get(&name.to_lowercase()) {
                Some(known) => Some(known.to_string()),
                None if opts.create_categories => match validate_category(name) {
                    Ok(name) => {
                        if !plan.new_categories.iter().any(|c| c.to_lowercase() == name.to_lowercase()) {
                            plan.new_categories.push(name.to_string());
                        }
                        Some(name.to_string())
                    }
                    Err(e) => {
                        plan.errors.push(RowError { line: row.line, message: e.to_string() });
                        continue;
                    }
                },
                None => default.clone(),
            },
            None => default.clone(),
        };
        let Some(category) = category else {
            plan.errors.push(RowError { line: row.line, message: "Choose a category for this row".into() });
            continue;
        };
        plan.rows.push(ImportRow { name: row.name, date: row.date, pence: row.pence, category });
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i16, m: i8, d: i8) -> Date {
        Date::new(y, m, d).unwrap()
    }

    #[test]
    fn dates() {
        use DateOrder::*;
        assert_eq!(parse_date("2026-11-02", Dmy), Ok(date(2026, 11, 2)));
        assert_eq!(parse_date("2026-11-02T14:30:00Z", Mdy), Ok(date(2026, 11, 2)));
        assert_eq!(parse_date("02/11/2026", Dmy), Ok(date(2026, 11, 2)));
        assert_eq!(parse_date("02/11/2026", Mdy), Ok(date(2026, 2, 11)));
        assert_eq!(parse_date("2.11.26", Dmy), Ok(date(2026, 11, 2)));
        assert_eq!(parse_date("02/11/2026 09:15", Dmy), Ok(date(2026, 11, 2)));
        assert_eq!(parse_date("2 Nov 2026", Mdy), Ok(date(2026, 11, 2)));
        assert_eq!(parse_date("02-Nov-2026", Mdy), Ok(date(2026, 11, 2)));
        assert_eq!(parse_date("November 2nd, 2026", Dmy), Ok(date(2026, 11, 2)));
        assert_eq!(parse_date("Mon 2 Nov 2026", Dmy), Ok(date(2026, 11, 2)));
        assert!(parse_date("31/02/2026", Dmy).unwrap_err().contains("real date"));
        assert!(parse_date("13/13/2026", Dmy).is_err());
        assert!(parse_date("01/268/2026", Dmy).is_err(), "268 must not wrap round to December");
        assert!(parse_date("268/01/2026", Mdy).is_err());
        assert!(parse_date("08:01:12", Dmy).is_err(), "a time is not a date");
        assert!(parse_date("2026:11:02", Dmy).is_err());
        assert!(parse_date("Lunch", Dmy).is_err());
        assert!(parse_date("12.50", Dmy).is_err());
        assert!(parse_date("", Dmy).is_err());
    }

    #[test]
    fn date_order_from_cells() {
        let cells = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(detect_date_order(cells(&["01/02/2026", "25/02/2026"]).iter(), DateOrder::Mdy), DateOrder::Dmy);
        assert_eq!(detect_date_order(cells(&["01/02/2026", "02/25/2026"]).iter(), DateOrder::Dmy), DateOrder::Mdy);
        assert_eq!(detect_date_order(cells(&["01/02/2026"]).iter(), DateOrder::Mdy), DateOrder::Mdy);
    }

    #[test]
    fn delimiters_and_quotes() {
        let t = Table::parse("\u{feff}Date,Description,Amount\r\n2026-11-02,\"Coffee, large\",3.20\r\n\r\n").unwrap();
        assert_eq!(t.delimiter, b',');
        assert_eq!(t.records.len(), 2);
        assert_eq!(t.records[1].1[1], "Coffee, large");
        assert_eq!(t.records.iter().map(|r| r.0).collect::<Vec<_>>(), vec![1, 2]);
        // Line numbers count blank lines and lines inside quoted cells.
        let t = Table::parse("a,b\r\n\r\n\"multi\nline\",x\r\nf,g\r\n").unwrap();
        assert_eq!(t.records.iter().map(|r| r.0).collect::<Vec<_>>(), vec![1, 3, 5]);

        let t = Table::parse("Datum;Beschreibung;Betrag\n02.11.2026;Mittagessen;12,50\n").unwrap();
        assert_eq!(t.delimiter, b';');
        // Without a header, the commas in the amounts mustn't win.
        let t = Table::parse("02.11.2026;Mittagessen;12,50\n03.11.2026;Kaffee;3,20\n").unwrap();
        assert_eq!(t.delimiter, b';');
        let t = Table::parse("a\tb\tc\n1\t2\t3\n").unwrap();
        assert_eq!(t.delimiter, b'\t');
        assert!(Table::parse("\n \n").is_err());
    }

    #[test]
    fn windows_1252_fallback() {
        let (text, legacy) = decode(b"Date,Amount\n2026-11-02,\xa312.50 \x80\n".to_vec());
        assert!(legacy);
        assert!(text.contains("£12.50 €"));
        assert_eq!(decode("£".as_bytes().to_vec()), ("£".to_string(), false));
    }

    #[test]
    fn guesses_from_headers() {
        let t = Table::parse("Transaction Date,Type,Description,Amount,Category\n02/11/2026,DEB,Lunch,-12.50,Food\n")
            .unwrap();
        let m = Mapping::guess(&t, DateOrder::Dmy);
        assert!(m.has_header);
        assert_eq!((m.date, m.name, m.amount, m.category), (Some(0), Some(2), Some(3), Some(4)));
        assert!(m.negate, "spending is negative in this file");
        let rows = m.rows(&t);
        assert_eq!(
            rows,
            vec![Ok(Parsed {
                line: 2,
                date: date(2026, 11, 2),
                name: "Lunch".into(),
                pence: 1250,
                category: Some("Food".into())
            })]
        );
    }

    #[test]
    fn time_column_is_not_the_date() {
        let t = Table::parse("08:01:12,02/11/2026,Lunch,12.50\n17:45:00,03/11/2026,Bus,2.80\n").unwrap();
        let m = Mapping::guess(&t, DateOrder::Dmy);
        assert_eq!(m.date, Some(1));
        assert_eq!(m.rows(&t)[0].as_ref().unwrap().date, date(2026, 11, 2));
    }

    #[test]
    fn guesses_without_headers() {
        let t = Table::parse("12.50,Lunch,11/25/2026\n3.20,Coffee,11/26/2026\n").unwrap();
        let m = Mapping::guess(&t, DateOrder::Dmy);
        assert!(!m.has_header);
        assert_eq!((m.date, m.name, m.amount, m.category), (Some(2), Some(1), Some(0), None));
        assert_eq!(m.date_order, DateOrder::Mdy);
        assert!(!m.negate);
        assert_eq!(m.rows(&t).len(), 2);
    }

    #[test]
    fn bad_rows_report_their_line() {
        let t = Table::parse("Date,Description,Amount\n2026-11-02,Lunch,abc\n2026-11-31,Tea,1\n2026-11-03,,1\n").unwrap();
        let m = Mapping::guess(&t, DateOrder::Dmy);
        let errors: Vec<u64> = m.rows(&t).into_iter().filter_map(|r| r.err()).map(|e| e.line).collect();
        assert_eq!(errors, vec![2, 3, 4]);
    }

    /// A bank export, all the way into a database, twice.
    #[test]
    fn imports_a_statement_once() {
        use crate::db::{SqliteStore, Store};
        let csv = "\u{feff}Date,Time,Type,Name,Category,Amount,Currency\r\n\
                   02/11/2026,08:01:12,Card payment,Pret A Manger,Eating out,-4.35,GBP\r\n\
                   02/11/2026,17:45:00,Card payment,\"TfL, Travel\",Transport,-2.80,GBP\r\n\
                   03/11/2026,12:00:00,Refund,Pret A Manger,Eating out,4.35,GBP\r\n";
        let table = Table::parse(csv).unwrap();
        let m = Mapping::guess(&table, DateOrder::Mdy);
        assert_eq!((m.date, m.name, m.amount, m.category), (Some(0), Some(3), Some(5), Some(4)));
        assert_eq!(m.date_order, DateOrder::Mdy, "nothing in the file says otherwise");
        assert!(m.negate);
        let m = Mapping { date_order: DateOrder::Dmy, ..m };

        let mut db = SqliteStore::open_in_memory().unwrap();
        let opts = Options { default_category: None, create_categories: true, skip_duplicates: true };
        let p = plan(m.rows(&table), &db.categories().unwrap(), &db.spending().unwrap(), &opts);
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        assert_eq!(p.new_categories, vec!["Eating out".to_string()]);
        db.import(&p.rows).unwrap();
        let spent: Vec<(i64, String)> = db.spending().unwrap().into_iter().map(|s| (s.pence, s.name)).collect();
        assert_eq!(spent, vec![(-435, "Pret A Manger".into()), (280, "TfL, Travel".into()), (435, "Pret A Manger".into())]);

        // Importing the same file again finds nothing new.
        let p = plan(m.rows(&table), &db.categories().unwrap(), &db.spending().unwrap(), &opts);
        assert_eq!((p.rows.len(), p.duplicates), (0, 3));
    }

    #[test]
    fn plans_categories_and_duplicates() {
        let cats = vec![Category { id: 1, name: "Food".into() }, Category { id: 2, name: "Other".into() }];
        let recorded = vec![Spending { line: 9, name: "Coffee".into(), date: date(2026, 11, 2), pence: 320, bucket: 1, owner: None, team: None }];
        let row = |name: &str, pence, category: Option<&str>| {
            Ok(Parsed { line: 1, date: date(2026, 11, 2), name: name.into(), pence, category: category.map(Into::into) })
        };
        let parsed = vec![
            row("coffee", 320, Some("food")),
            row("Coffee", 320, Some("FOOD")),
            row("Bus", 250, Some("Transport")),
            row("Train", 900, Some("transport")),
            row("Gift", 1000, None),
            Err(RowError { line: 7, message: "bad".into() }),
        ];
        let mut opts = Options { default_category: Some(2), create_categories: true, skip_duplicates: true };
        let p = plan(parsed.clone(), &cats, &recorded, &opts);
        assert_eq!(p.duplicates, 1);
        assert_eq!(p.errors.len(), 1);
        assert_eq!(p.new_categories, vec!["Transport".to_string()]);
        let names: Vec<_> = p.rows.iter().map(|r| (r.name.as_str(), r.category.as_str())).collect();
        assert_eq!(
            names,
            vec![("Coffee", "Food"), ("Bus", "Transport"), ("Train", "transport"), ("Gift", "Other")]
        );

        opts.create_categories = false;
        opts.skip_duplicates = false;
        let p = plan(parsed, &cats, &recorded, &opts);
        assert_eq!(p.duplicates, 0);
        assert!(p.new_categories.is_empty());
        assert_eq!(p.rows[2].category, "Other");
    }
}
