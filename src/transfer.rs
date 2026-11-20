//! Spending in and out of files that other programs can read: CSV, for
//! spreadsheets, and JSON. The app reads back what it writes, and CSV files
//! from elsewhere as long as their first row names the columns.
//!
//! Each entry has a date (written YYYY-MM-DD), a name (what was bought), an
//! amount and a category. Amounts are written as plain numbers, such as 12.50
//! or -3.99, whatever the locale, so that any program can read them. Reading,
//! amounts with currency symbols and the locale's own separators are understood
//! too, and dates in the locale's order. JSON also lists every category, so
//! that ones with no spending in them come back as well.
//!
//! A file is checked in full before anything is saved, and a problem names the
//! line (CSV) or entry (JSON) it is on.

use std::path::Path;

use chrono::{Datelike, NaiveDate};
use serde_json::{Value, json};

use crate::db::{NewSpending, Spending};
use crate::locale::{LocaleFormat, parse_date};
use crate::money;

/// Bigger than anyone's spending, and small enough to read without a pause.
pub const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;
/// The database's limits, as on the Spending and Categories tabs.
const MAX_NAME_CHARS: usize = 160;
const MAX_CATEGORY_CHARS: usize = 32;

/// The column names written, and the other names accepted for each.
const DATE_COLUMNS: &[&str] = &["date", "day", "when"];
const NAME_COLUMNS: &[&str] = &["name", "what you bought", "description", "item", "payee", "details"];
const AMOUNT_COLUMNS: &[&str] = &["amount", "price", "cost", "value"];
const CATEGORY_COLUMNS: &[&str] = &["category", "bucket"];

/// Spreadsheets run text that starts with one of these as a formula, so text
/// that does is written after an apostrophe, which they hide, and is read
/// back without it.
const FORMULA_STARTS: &[char] = &['=', '+', '-', '@', '\t', '\r'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    Csv,
    Json,
}

impl DataFormat {
    /// The format a file's extension says it is in.
    pub fn of(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "csv" | "txt" => Some(DataFormat::Csv),
            "json" => Some(DataFormat::Json),
            _ => None,
        }
    }
}

/// What a file holds, checked and ready to save.
#[derive(Debug, Default, PartialEq)]
pub struct Imported {
    pub entries: Vec<NewSpending>,
    /// Categories the file lists, whether or not any entry is in them.
    pub categories: Vec<String>,
}

impl Imported {
    /// Every category named, listed or used, with repeats.
    pub fn category_names(&self) -> impl Iterator<Item = &str> {
        self.categories.iter().map(String::as_str).chain(self.entries.iter().map(|e| e.category.as_str()))
    }
}

/// Reads a file of either format. The locale and today are for amounts and
/// dates written the way people type them.
pub fn read(format: DataFormat, bytes: &[u8], fmt: &LocaleFormat, today: NaiveDate) -> Result<Imported, String> {
    let text = decode(bytes);
    let imported = match format {
        DataFormat::Csv => read_csv(&text, fmt, today)?,
        DataFormat::Json => read_json(&text, fmt, today)?,
    };
    if imported.entries.is_empty() && imported.categories.is_empty() {
        return Err("The file has no spending in it.".to_owned());
    }
    Ok(imported)
}

/// UTF-8, without a byte order mark. Spreadsheets on Windows may save in
/// Windows-1252 instead, which is read as that.
fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => bytes.iter().map(|&b| windows_1252(b)).collect(),
    }
}

fn windows_1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘', '’',
        '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9f => HIGH[usize::from(b - 0x80)],
        _ => char::from(b),
    }
}

// ----- checking each value ----------------------------------------------------

/// Text from a file as a single line: control characters, such as line breaks
/// inside quotes, become spaces, and runs of spaces become one.
fn one_line(text: &str) -> String {
    text.split(|c: char| c.is_whitespace() || c.is_control()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Text as written for a spreadsheet: after an apostrophe if it would
/// otherwise run as a formula.
fn defuse(text: &str) -> String {
    if text.starts_with(FORMULA_STARTS) { format!("'{text}") } else { text.to_owned() }
}

fn undefuse(text: &str) -> &str {
    match text.strip_prefix('\'') {
        Some(rest) if rest.starts_with(FORMULA_STARTS) => rest,
        _ => text,
    }
}

fn read_name(text: &str) -> Result<String, String> {
    let name = one_line(undefuse(text.trim()));
    if name.is_empty() {
        return Err("doesn't say what was bought".to_owned());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!("says what was bought in more than {MAX_NAME_CHARS} characters"));
    }
    Ok(name)
}

fn read_category(text: &str) -> Result<String, String> {
    let name = one_line(undefuse(text.trim()));
    if name.is_empty() {
        return Err("has no category".to_owned());
    }
    if name.chars().count() > MAX_CATEGORY_CHARS {
        return Err(format!("has a category, \"{name}\", longer than {MAX_CATEGORY_CHARS} characters"));
    }
    Ok(name)
}

/// A plain number such as 12.50 first, as this app writes, and then as typed
/// in the locale, with its currency symbol and separators.
fn read_amount(text: &str, fmt: &LocaleFormat) -> Result<i64, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("has no amount".to_owned());
    }
    money::parse_canonical(text)
        .or_else(|_| fmt.parse_money(text))
        .map_err(|_| format!("has an amount, \"{text}\", that isn't a number such as 12.50"))
}

/// YYYY-MM-DD first, as this app writes, and then in the locale's order.
fn read_date(text: &str, fmt: &LocaleFormat, today: NaiveDate) -> Result<NaiveDate, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("has no date".to_owned());
    }
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .or_else(|_| parse_date(text, today, fmt.date_order))
        // The database keeps years of four digits.
        .ok()
        .filter(|d| (0..=9999).contains(&d.year()))
        .ok_or_else(|| format!("has a date, \"{text}\", that isn't understood. Dates such as 2026-11-18 always work"))
}

/// "Line 4 has no amount.", and how many other problems there are.
fn problems(found: Vec<String>) -> Result<(), String> {
    let Some(first) = found.first() else { return Ok(()) };
    let more = match found.len() - 1 {
        0 => String::new(),
        1 => " There is 1 more problem like it.".to_owned(),
        n => format!(" There are {n} more problems like it."),
    };
    Err(format!("{first}.{more}"))
}

// ----- CSV ------------------------------------------------------------------

/// Spending as CSV, with a header row, in UTF-8 with a byte order mark so that
/// spreadsheets on Windows show symbols such as £ correctly.
pub fn to_csv(entries: &[Spending]) -> Vec<u8> {
    let mut out = b"\xEF\xBB\xBF".to_vec();
    {
        let mut writer = csv::WriterBuilder::new().terminator(csv::Terminator::CRLF).from_writer(&mut out);
        let rows = std::iter::once(["Date".to_owned(), "Name".to_owned(), "Amount".to_owned(), "Category".to_owned()])
            .chain(entries.iter().map(|e| {
                [e.date.format("%Y-%m-%d").to_string(), defuse(&e.name), money::canonical(e.pence), defuse(&e.category)]
            }));
        for row in rows {
            writer.write_record(&row).expect("writing to memory can't fail");
        }
        writer.flush().expect("writing to memory can't fail");
    }
    out
}

/// The separator a CSV file uses: whichever of comma, semicolon (as
/// spreadsheets in much of Europe save) or tab is most common in its first line.
fn delimiter(text: &str) -> u8 {
    let first = text.lines().next().unwrap_or("");
    b",;\t".iter().copied().max_by_key(|d| (first.bytes().filter(|b| b == d).count(), *d == b',')).unwrap_or(b',')
}

fn read_csv(text: &str, fmt: &LocaleFormat, today: NaiveDate) -> Result<Imported, String> {
    let mut reader =
        csv::ReaderBuilder::new().delimiter(delimiter(text)).flexible(true).has_headers(true).from_reader(text.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| format!("The file couldn't be read as CSV: {e}."))?
        .iter()
        .map(|h| one_line(h).to_lowercase())
        .collect();
    let column = |names: &[&str]| headers.iter().position(|h| names.contains(&h.as_str()));
    let (Some(date), Some(name), Some(amount), Some(category)) =
        (column(DATE_COLUMNS), column(NAME_COLUMNS), column(AMOUNT_COLUMNS), column(CATEGORY_COLUMNS))
    else {
        return Err(format!(
            "The first line of the file must name its columns: Date, Name, Amount and Category. It has: {}.",
            if headers.iter().all(String::is_empty) { "nothing".to_owned() } else { headers.join(", ") }
        ));
    };
    let mut entries = Vec::new();
    let mut found = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| format!("The file couldn't be read as CSV: {e}."))?;
        let line = record.position().map_or(0, |p| p.line());
        if record.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        let field = |i: usize| record.get(i).unwrap_or("");
        let entry = (|| -> Result<NewSpending, String> {
            Ok(NewSpending {
                date: read_date(field(date), fmt, today)?,
                name: read_name(field(name))?,
                pence: read_amount(field(amount), fmt)?,
                category: read_category(field(category))?,
            })
        })();
        match entry {
            Ok(entry) => entries.push(entry),
            Err(problem) => found.push(format!("Line {line} {problem}")),
        }
    }
    problems(found)?;
    Ok(Imported { entries, categories: Vec::new() })
}

// ----- JSON -----------------------------------------------------------------

/// Spending, and every category, as JSON.
pub fn to_json(entries: &[Spending], categories: &[String], today: NaiveDate) -> Vec<u8> {
    let spending: Vec<Value> = entries
        .iter()
        .map(|e| {
            json!({
                "date": e.date.format("%Y-%m-%d").to_string(),
                "name": e.name,
                "amount": money::canonical(e.pence),
                "category": e.category,
            })
        })
        .collect();
    let document = json!({
        "app": crate::APP_TITLE,
        "version": env!("CARGO_PKG_VERSION"),
        "exported": today.format("%Y-%m-%d").to_string(),
        "categories": categories,
        "spending": spending,
    });
    let mut out = serde_json::to_vec_pretty(&document).expect("JSON values always serialise");
    out.push(b'\n');
    out
}

fn read_json(text: &str, fmt: &LocaleFormat, today: NaiveDate) -> Result<Imported, String> {
    let document: Value = serde_json::from_str(text).map_err(|e| format!("The file isn't valid JSON: {e}."))?;
    let (spending, listed) = match &document {
        Value::Array(items) => (items.as_slice(), None),
        Value::Object(fields) => match fields.get("spending") {
            Some(Value::Array(items)) => (items.as_slice(), fields.get("categories")),
            _ => return Err("The file has no \"spending\" list of entries.".to_owned()),
        },
        _ => return Err("The file must be a list of entries, or have a \"spending\" list of them.".to_owned()),
    };

    let mut found = Vec::new();
    let mut categories = Vec::new();
    match listed {
        None | Some(Value::Null) => {}
        Some(Value::Array(names)) => {
            for (i, name) in names.iter().enumerate() {
                match name.as_str().ok_or_else(|| "isn't text".to_owned()).and_then(read_category) {
                    Ok(name) => categories.push(name),
                    Err(problem) => found.push(format!("Category {} in the categories list {problem}", i + 1)),
                }
            }
        }
        Some(_) => found.push("The \"categories\" value isn't a list".to_owned()),
    }

    let mut entries = Vec::new();
    for (i, item) in spending.iter().enumerate() {
        let entry = (|| -> Result<NewSpending, String> {
            let Value::Object(fields) = item else { return Err("isn't an object with a date, name, amount and category".to_owned()) };
            let text = |keys: &[&str], missing: &str| -> Result<String, String> {
                match keys.iter().find_map(|k| fields.get(*k)) {
                    Some(Value::String(s)) => Ok(s.clone()),
                    Some(Value::Number(n)) => Ok(n.to_string()),
                    None | Some(Value::Null) => Err(missing.to_owned()),
                    Some(_) => Err(format!("has a {} that isn't text", keys[0])),
                }
            };
            Ok(NewSpending {
                date: read_date(&text(&["date"], "has no date")?, fmt, today)?,
                name: read_name(&text(&["name", "description"], "doesn't say what was bought")?)?,
                pence: match fields.get("amount") {
                    // A number is never in the locale's format.
                    Some(Value::Number(n)) => money::parse_canonical(&n.to_string())
                        .map_err(|_| format!("has an amount, {n}, that isn't a number such as 12.50"))?,
                    _ => read_amount(&text(&["amount"], "has no amount")?, fmt)?,
                },
                category: read_category(&text(&["category"], "has no category")?)?,
            })
        })();
        match entry {
            Ok(entry) => entries.push(entry),
            Err(problem) => found.push(format!("Entry {} {problem}", i + 1)),
        }
    }
    problems(found)?;
    Ok(Imported { entries, categories })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Locale;

    fn gb() -> LocaleFormat {
        LocaleFormat::new(Locale::en_GB, "test")
    }

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn today() -> NaiveDate {
        d("2026-11-20")
    }

    fn spending(line: i64, date: &str, name: &str, pence: i64, category: &str) -> Spending {
        Spending { line, name: name.to_owned(), date: d(date), pence, category: category.to_owned() }
    }

    fn new(date: &str, name: &str, pence: i64, category: &str) -> NewSpending {
        NewSpending { date: d(date), name: name.to_owned(), pence, category: category.to_owned() }
    }

    fn csv(text: &str) -> Result<Imported, String> {
        read(DataFormat::Csv, text.as_bytes(), &gb(), today())
    }

    fn awkward() -> Vec<Spending> {
        vec![
            spending(1, "2026-11-18", "Tea, \"strong\"", 250, "Food and Drink"),
            spending(2, "2026-11-19", "=SUM(A1:A9)", -399, "Refunds; misc"),
            spending(3, "2026-11-20", "-5% off", 99_999_999, "+Plus"),
            spending(4, "2026-11-20", "Café £ € ₹", 1, "Food and Drink"),
        ]
    }

    #[test]
    fn csv_round_trips() {
        let written = to_csv(&awkward());
        let text = String::from_utf8(written.clone()).unwrap();
        assert!(text.starts_with("\u{feff}Date,Name,Amount,Category\r\n2026-11-18,\"Tea, \"\"strong\"\"\",2.50,"), "{text}");
        // Nothing a spreadsheet would run as a formula.
        assert!(text.contains(",'=SUM(A1:A9),-3.99,"), "{text}");
        assert!(text.contains(",'-5% off,999999.99,'+Plus"), "{text}");
        let back = read(DataFormat::Csv, &written, &gb(), today()).unwrap();
        let expected: Vec<NewSpending> = awkward().iter().map(|e| new(&e.date.to_string(), &e.name, e.pence, &e.category)).collect();
        assert_eq!(back.entries, expected);
    }

    #[test]
    fn json_round_trips_with_every_category() {
        let categories = vec!["Food and Drink".to_owned(), "Refunds; misc".to_owned(), "+Plus".to_owned(), "Empty".to_owned()];
        let written = to_json(&awkward(), &categories, today());
        let value: Value = serde_json::from_slice(&written).unwrap();
        assert_eq!(value["spending"][1]["amount"], "-3.99");
        assert_eq!(value["exported"], "2026-11-20");
        let back = read(DataFormat::Json, &written, &gb(), today()).unwrap();
        assert_eq!(back.categories, categories);
        assert_eq!(back.entries.len(), 4);
        assert_eq!(back.entries[1], new("2026-11-19", "=SUM(A1:A9)", -399, "Refunds; misc"));
    }

    #[test]
    fn csv_from_elsewhere_is_understood() {
        // Another order, other names, extra columns, and amounts as typed.
        let file = "Description,Category,Notes,Cost,Day\n\
                    Bus,Transport,,£1.80,18/11/2026\n\
                    \"Two\nlines\",Food and Drink,x,\"1,234.50\",2026-11-01\n\
                    ,,,,\n";
        let got = csv(file).unwrap();
        assert_eq!(got.entries, [new("2026-11-18", "Bus", 180, "Transport"), new("2026-11-01", "Two lines", 123_450, "Food and Drink")]);

        // A European spreadsheet's semicolons and decimal commas.
        let de = LocaleFormat::new(Locale::de_DE, "test");
        let file = "Datum;Name;Amount;Category\n";
        assert!(read(DataFormat::Csv, file.as_bytes(), &de, today()).unwrap_err().ends_with("It has: datum, name, amount, category."));
        let file = "Date;Name;Amount;Category\n18.11.2026;Kaffee;3,20;Essen\n";
        assert_eq!(read(DataFormat::Csv, file.as_bytes(), &de, today()).unwrap().entries, [new("2026-11-18", "Kaffee", 320, "Essen")]);

        // Windows-1252, as older spreadsheets save.
        let file = b"Date,Name,Amount,Category\n2026-11-18,Caf\xe9,\xa32.00,Food\n";
        assert_eq!(read(DataFormat::Csv, file, &gb(), today()).unwrap().entries[0].name, "Café");
    }

    #[test]
    fn csv_problems_are_named_by_line_and_nothing_is_kept() {
        let err = csv("Name,Amount\nTea,1\n").unwrap_err();
        assert_eq!(err, "The first line of the file must name its columns: Date, Name, Amount and Category. It has: name, amount.");
        let err = csv("Date,Name,Amount,Category\n2026-11-18,Tea,lots,Food\n2026-11-31,Tea,1,Food\n2026-11-18,,1,Food\n").unwrap_err();
        assert_eq!(
            err,
            "Line 2 has an amount, \"lots\", that isn't a number such as 12.50. There are 2 more problems like it.",
        );
        let err = csv(&format!("Date,Name,Amount,Category\n2026-11-18,{},1,Food\n", "x".repeat(161))).unwrap_err();
        assert!(err.starts_with("Line 2 says what was bought in more than 160 characters"), "{err}");
        for date in ["12345-01-01", "-0001-01-01"] {
            let err = csv(&format!("Date,Name,Amount,Category\n{date},Tea,1,Food\n")).unwrap_err();
            assert!(err.starts_with(&format!("Line 2 has a date, \"{date}\"")), "{err}");
        }
        let err = csv("Date,Name,Amount,Category\n2026-11-18,Tea,1000000,Food\n").unwrap_err();
        assert!(err.starts_with("Line 2 has an amount"), "{err}");
        assert_eq!(csv("Date,Name,Amount,Category\n").unwrap_err(), "The file has no spending in it.");
        assert_eq!(csv("").unwrap_err().split(" It has").next().unwrap(), "The first line of the file must name its columns: Date, Name, Amount and Category.");
    }

    #[test]
    fn json_from_elsewhere_is_understood() {
        let file = r#"[{"date": "2026-11-18", "description": "Bus", "amount": 1.8, "category": "Transport"},
                       {"date": "18/11/2026", "name": "Tea", "amount": "£2.50", "category": "Food"}]"#;
        let got = read(DataFormat::Json, file.as_bytes(), &gb(), today()).unwrap();
        assert_eq!(got.entries, [new("2026-11-18", "Bus", 180, "Transport"), new("2026-11-18", "Tea", 250, "Food")]);

        let bad = |text: &str| read(DataFormat::Json, text.as_bytes(), &gb(), today()).unwrap_err();
        assert!(bad("{").starts_with("The file isn't valid JSON"));
        assert_eq!(bad("{}"), "The file has no \"spending\" list of entries.");
        assert_eq!(bad("3"), "The file must be a list of entries, or have a \"spending\" list of them.");
        assert_eq!(bad(r#"[{"date": "2026-11-18", "name": "Tea", "amount": 2.555, "category": "Food"}]"#),
            "Entry 1 has an amount, 2.555, that isn't a number such as 12.50.");
        assert_eq!(bad(r#"[1, {"date": true}]"#),
            "Entry 1 isn't an object with a date, name, amount and category. There is 1 more problem like it.");
        assert_eq!(bad(r#"{"categories": [""], "spending": []}"#), "Category 1 in the categories list has no category.");
    }

    #[test]
    fn formats_come_from_the_extension() {
        assert_eq!(DataFormat::of(Path::new("a/b.CSV")), Some(DataFormat::Csv));
        assert_eq!(DataFormat::of(Path::new("b.json")), Some(DataFormat::Json));
        assert_eq!(DataFormat::of(Path::new("b.pdf")), None);
        assert_eq!(DataFormat::of(Path::new("b")), None);
    }
}
