//! Importing and exporting: spending as CSV, everything as a JSON backup,
//! and everything as a MariaDB SQL script in the schema the app came from.

use crate::db::{Category, Db, Entry};
use crate::money;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// What kind of file, from its extension.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Csv,
    Json,
    Sql,
}

impl Format {
    pub fn of(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "csv" => Some(Format::Csv),
            "json" => Some(Format::Json),
            "sql" => Some(Format::Sql),
            _ => None,
        }
    }
}

// ----- CSV -----------------------------------------------------------------

const CSV_HEADER: [&str; 4] = ["name", "date", "price", "category"];

/// Writes every entry as CSV: name, date, price and category name.
pub fn export_csv(db: &Db, path: &Path) -> Result<usize> {
    let categories = db.categories()?;
    let entries = db.all_entries()?;
    let mut out = csv::Writer::from_writer(Vec::new());
    out.write_record(CSV_HEADER)?;
    for e in &entries {
        let category = categories.iter().find(|c| c.id == e.bucket).map_or("", |c| c.short.as_str());
        let (date, price) = (e.date.to_string(), money::plain(e.price));
        out.write_record([as_text(&e.name).as_ref(), &date, &price, as_text(category).as_ref()])?;
    }
    write(path, &out.into_inner()?)?;
    Ok(entries.len())
}

/// Reads entries from CSV with name, date, price and category columns, in
/// any order. "bucket" is also accepted for the category, and "amount" for
/// the price. Every row is checked before anything is added.
pub fn read_csv(text: &str) -> Result<Vec<(Entry, String)>> {
    let mut reader = csv::ReaderBuilder::new().trim(csv::Trim::All).flexible(true).from_reader(text.as_bytes());
    let headers: Vec<String> = reader.headers()?.iter().map(|h| h.trim_start_matches('\u{feff}').to_lowercase()).collect();
    let column = |names: &[&str]| headers.iter().position(|h| names.contains(&h.as_str()));
    let (Some(name), Some(date), Some(price), Some(category)) = (
        column(&["name"]),
        column(&["date"]),
        column(&["price", "amount"]),
        column(&["category", "bucket"]),
    ) else {
        bail!(t!("csv-header"));
    };
    let mut rows = Vec::new();
    for (i, record) in reader.records().enumerate() {
        // Row 1 is the header.
        let row = i + 2;
        let record = record.with_context(|| t!("csv-row", row = row))?;
        if record.iter().all(str::is_empty) {
            continue;
        }
        let field = |i: usize| record.get(i).unwrap_or("");
        let row_error = |e: String| anyhow::anyhow!(t!("csv-row-error", row = row, error = e));
        let parsed_date = money::parse_date(field(date)).map_err(row_error)?;
        let parsed_price = money::parse(field(price), money::Numbers::PLAIN).map_err(row_error)?;
        let entry = Entry { line: 0, name: from_text(field(name)).to_owned(), date: parsed_date, price: parsed_price, bucket: 0 };
        if entry.name.is_empty() {
            bail!(t!("csv-name-empty", row = row));
        }
        if entry.name.chars().count() > crate::db::MAX_NAME_CHARS {
            bail!(t!("csv-name-too-long", row = row, max = crate::db::MAX_NAME_CHARS));
        }
        let category = crate::db::check_category_name(from_text(field(category))).map_err(|e| row_error(e.to_string()))?;
        rows.push((entry, category));
    }
    Ok(rows)
}

/// Whether a spreadsheet would read `cell` as a formula rather than text:
/// it starts with = + - @ or a tab or carriage return. Also true after a
/// quote added by `as_text`, so a name that really starts with '= is
/// marked too, and comes back unchanged.
fn reads_as_formula(cell: &str) -> bool {
    match cell.strip_prefix('\'') {
        Some(rest) => reads_as_formula(rest),
        None => cell.starts_with(['=', '+', '-', '@', '\t', '\r']),
    }
}

/// A name for a CSV file, with a quote in front if a spreadsheet would
/// otherwise run it as a formula, such as "=HYPERLINK(...)". Spreadsheets
/// show the quote as marking text; `from_text` takes it off again.
fn as_text(cell: &str) -> std::borrow::Cow<'_, str> {
    if reads_as_formula(cell) { format!("'{cell}").into() } else { cell.into() }
}

/// A name from a CSV file without the quote `as_text` added.
fn from_text(cell: &str) -> &str {
    match cell.strip_prefix('\'') {
        Some(rest) if reads_as_formula(rest) => rest,
        _ => cell,
    }
}

pub struct CsvImport {
    pub entries: usize,
    pub new_categories: usize,
}

/// Adds the entries in a CSV file, creating categories it names that don't
/// exist yet.
pub fn import_csv(db: &mut Db, path: &Path) -> Result<CsvImport> {
    let text = read_text(path)?;
    let rows = read_csv(&text)?;
    let new_categories = db.add_entries_by_category_name(&rows)?;
    Ok(CsvImport { entries: rows.len(), new_categories })
}

// ----- JSON ----------------------------------------------------------------

/// Identifies a JSON backup made by this app.
const BACKUP_KIND: &str = "open-budget-tracker";

#[derive(Serialize, Deserialize)]
struct Backup {
    kind: String,
    version: u32,
    categories: Vec<BackupCategory>,
    spending: Vec<BackupEntry>,
}

#[derive(Serialize, Deserialize)]
struct BackupCategory {
    id: i64,
    short: String,
    /// The monthly budget, such as "200.00".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    budget: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct BackupEntry {
    line: i64,
    name: String,
    date: String,
    /// Written as text, such as "12.50", so no precision is lost.
    price: String,
    bucket: i64,
}

pub fn export_json(db: &Db, path: &Path) -> Result<(usize, usize)> {
    let categories = db.categories()?;
    let entries = db.all_entries()?;
    let backup = Backup {
        kind: BACKUP_KIND.into(),
        version: 1,
        categories: categories
            .iter()
            .map(|c| BackupCategory { id: c.id, short: c.short.clone(), budget: c.budget.map(money::plain) })
            .collect(),
        spending: entries
            .iter()
            .map(|e| BackupEntry {
                line: e.line,
                name: e.name.clone(),
                date: e.date.to_string(),
                price: money::plain(e.price),
                bucket: e.bucket,
            })
            .collect(),
    };
    write(path, &serde_json::to_vec_pretty(&backup)?)?;
    Ok((categories.len(), entries.len()))
}

/// What's in a JSON backup, checked and ready to go into the database.
pub struct BackupContents {
    pub categories: Vec<Category>,
    pub entries: Vec<Entry>,
}

pub fn read_json(text: &str) -> Result<BackupContents> {
    let not_ours = t!("backup-not-ours");
    let backup: Backup = serde_json::from_str(text).context(not_ours.trim_end_matches('.').to_owned())?;
    if backup.kind != BACKUP_KIND {
        bail!(not_ours);
    }
    if backup.version > 1 {
        bail!(t!("backup-newer"));
    }
    let categories = backup
        .categories
        .into_iter()
        .map(|c| {
            let budget = c.budget.as_deref().map(|b| money::parse(b, money::Numbers::PLAIN)).transpose().map_err(anyhow::Error::msg)?;
            Ok(Category { id: c.id, short: c.short, budget })
        })
        .collect::<Result<Vec<_>>>()?;
    let entries = backup
        .spending
        .into_iter()
        .map(|e| {
            let context = |err: String| anyhow::anyhow!(t!("backup-entry-error", line = e.line, error = err));
            Ok(Entry {
                line: e.line,
                date: money::parse_date(&e.date).map_err(context)?,
                price: money::parse(&e.price, money::Numbers::PLAIN).map_err(context)?,
                name: e.name,
                bucket: e.bucket,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(BackupContents { categories, entries })
}

/// Reads a JSON backup. With `replace`, it replaces everything; otherwise
/// its entries are added, matching categories by name.
pub fn import_json(db: &mut Db, path: &Path, replace: bool) -> Result<(usize, usize)> {
    let contents = read_json(&read_text(path)?)?;
    if replace {
        db.replace_all(&contents.categories, &contents.entries)?;
        return Ok((contents.categories.len(), contents.entries.len()));
    }
    let rows = contents
        .entries
        .into_iter()
        .map(|e| {
            let category = contents
                .categories
                .iter()
                .find(|c| c.id == e.bucket)
                .map(|c| c.short.clone())
                .with_context(|| t!("backup-missing-category", line = e.line, category = e.bucket))?;
            Ok((e, category))
        })
        .collect::<Result<Vec<_>>>()?;
    let created = db.add_entries_by_category_name(&rows)?;
    Ok((created, rows.len()))
}

// ----- SQL -----------------------------------------------------------------

/// Writes a MariaDB script that recreates the tables and their contents,
/// using the original schema's types (prices as DECIMAL(8,2)).
///
/// The tables are utf8mb4, so any name loads. Category names compare
/// exactly (utf8mb4_bin): SQLite only ignores the case of ASCII letters, so
/// names such as "Café" and "CAFÉ", or "Cafe" and "Café", can both exist,
/// and MariaDB's usual collation would call them duplicates.
pub fn export_sql(db: &Db, path: &Path) -> Result<usize> {
    let categories = db.categories()?;
    let entries = db.all_entries()?;
    // The same tables the app makes on a server, so an export can be loaded
    // and then used from the app.
    let mut sql = format!(
        "-- Exported from Open Budget Tracker.\nSET NAMES utf8mb4;\n\n{};\n",
        crate::db::MARIADB_SCHEMA.join(";\n\n")
    );
    if !categories.is_empty() {
        let rows: Vec<String> = categories.iter().map(|c| format!("({}, {})", c.id, sql_string(&c.short))).collect();
        sql.push_str(&format!("\nINSERT INTO `category` (`id`, `short`) VALUES\n{};\n", rows.join(",\n")));
    }
    let budgets: Vec<String> = categories
        .iter()
        .filter_map(|c| Some(format!("({}, {})", c.id, money::plain(c.budget?))))
        .collect();
    if !budgets.is_empty() {
        sql.push_str(&format!("\nINSERT INTO `budget` (`bucket`, `monthly`) VALUES\n{};\n", budgets.join(",\n")));
    }
    // In batches, to keep each statement a sensible size.
    for chunk in entries.chunks(500) {
        let rows: Vec<String> = chunk
            .iter()
            .map(|e| format!("({}, {}, '{}', {}, {})", e.line, sql_string(&e.name), e.date, money::plain(e.price), e.bucket))
            .collect();
        sql.push_str(&format!(
            "\nINSERT INTO `spending` (`line`, `name`, `date`, `price`, `bucket`) VALUES\n{};\n",
            rows.join(",\n")
        ));
    }
    write(path, sql.as_bytes())?;
    Ok(entries.len())
}

/// A MariaDB string expression for `text` that reads the same in every SQL
/// mode. Quotes are doubled, which every mode understands, and backslash
/// escapes are never used: with NO_BACKSLASH_ESCAPES they would be kept as
/// they are. A backslash, which starts an escape in the usual mode, is
/// written as CHAR(92), and so are NUL and Ctrl+Z, which can end a file
/// early, joined to the rest with CONCAT. Line breaks need nothing.
fn sql_string(text: &str) -> String {
    let mut parts = Vec::new();
    let mut quoted = String::new();
    for c in text.chars() {
        match c {
            '\\' | '\0' | '\u{1a}' => {
                if !quoted.is_empty() {
                    parts.push(format!("'{}'", std::mem::take(&mut quoted)));
                }
                parts.push(format!("CHAR({} USING utf8mb4)", u32::from(c)));
            }
            '\'' => quoted.push_str("''"),
            c => quoted.push(c),
        }
    }
    if !quoted.is_empty() || parts.is_empty() {
        parts.push(format!("'{quoted}'"));
    }
    match parts.as_slice() {
        [one] => one.clone(),
        _ => format!("CONCAT({})", parts.join(", ")),
    }
}

// ----- files ---------------------------------------------------------------

/// The largest file imported, so a wrong choice can't use all the memory.
const MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;

fn read_text(path: &Path) -> Result<String> {
    let size = std::fs::metadata(path).with_context(|| t!("error-open-file", path = path.display().to_string()))?.len();
    if size > MAX_IMPORT_BYTES {
        bail!(t!("file-too-large", file = path.display().to_string()));
    }
    let bytes = std::fs::read(path).with_context(|| t!("error-read-file", path = path.display().to_string()))?;
    String::from_utf8(bytes).map_err(|_| anyhow::anyhow!(t!("file-not-utf8", file = path.display().to_string())))
}

/// Writes the whole file or, if that fails, leaves what was there.
fn write(path: &Path, contents: &[u8]) -> Result<()> {
    crate::paths::write_file(path, contents).with_context(|| t!("error-write-file", path = path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Db {
        let mut db = Db::open_in_memory().unwrap();
        db.update_category(23, "Food and Drink", Some(20_000)).unwrap();
        let rows = vec![
            (Entry { line: 0, name: "Fish, chips".into(), date: "2026-10-02".parse().unwrap(), price: 1250, bucket: 0 }, "Food and Drink".to_owned()),
            (Entry { line: 0, name: "O'Neill's \"pub\"".into(), date: "2026-10-03".parse().unwrap(), price: -300, bucket: 0 }, "Pubs".to_owned()),
        ];
        assert_eq!(db.add_entries_by_category_name(&rows).unwrap(), 1);
        db
    }

    #[test]
    fn csv_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spending.csv");
        let db = sample();
        assert_eq!(export_csv(&db, &path).unwrap(), 2);
        let mut fresh = Db::open_in_memory().unwrap();
        let imported = import_csv(&mut fresh, &path).unwrap();
        assert_eq!((imported.entries, imported.new_categories), (2, 1));
        let names = |db: &Db| db.all_entries().unwrap().into_iter().map(|e| (e.name, e.price)).collect::<Vec<_>>();
        assert_eq!(names(&fresh), names(&db));
    }

    /// Names a spreadsheet would run as formulas are written as text, and
    /// read back as they were.
    #[test]
    fn csv_names_never_run_as_formulas() {
        let names = ["=HYPERLINK(\"http://x\",\"y\")", "+1", "-refund", "@SUM(A1)", "'=already quoted", "''=two", "'plain", "plain"];
        for name in names {
            let written = as_text(name);
            assert!(!reads_as_formula(&written) || written.starts_with('\''), "{name}");
            assert!(!["=", "+", "-", "@"].iter().any(|c| written.starts_with(c)), "{name} is written as {written}");
            assert_eq!(from_text(&written), name, "{name} comes back");
        }
        assert_eq!(as_text("plain"), "plain");
        assert_eq!(as_text("'plain"), "'plain", "a quote before ordinary text is left alone");

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("formulas.csv");
        let mut db = Db::open_in_memory().unwrap();
        let rows: Vec<(Entry, String)> = names
            .iter()
            .map(|n| (Entry { line: 0, name: (*n).into(), date: "2026-10-02".parse().unwrap(), price: -100, bucket: 0 }, format!("={n}").chars().take(32).collect()))
            .collect();
        db.add_entries_by_category_name(&rows).unwrap();
        export_csv(&db, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.lines().skip(1).all(|l| !l.starts_with(['=', '+', '-', '@'])), "{text}");
        assert!(text.contains(",-1.00,"), "amounts are left as numbers: {text}");
        let mut fresh = Db::open_in_memory().unwrap();
        import_csv(&mut fresh, &path).unwrap();
        let named = |db: &Db| {
            let categories = db.categories().unwrap();
            let mut all: Vec<(String, String)> = db
                .all_entries()
                .unwrap()
                .into_iter()
                .map(|e| (e.name, categories.iter().find(|c| c.id == e.bucket).unwrap().short.clone()))
                .collect();
            all.sort();
            all
        };
        assert_eq!(named(&fresh), named(&db));
    }

    #[test]
    fn csv_columns_in_any_order_and_errors_name_the_row() {
        let rows = read_csv("\u{feff}Category,Amount,Name,Date\nTransport,£2.50,Bus,2026-10-01\n,,,\n").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].0.price, rows[0].1.as_str()), (250, "Transport"));
        let err = read_csv("name,date,price,category\nBus,2026-10-01,2.50,Transport\nTrain,yesterday,5,Transport\n")
            .unwrap_err();
        assert!(err.to_string().starts_with("Row 3:"), "{err}");
        let err = read_csv("name,date,price,category
Bus,2026-10-01,2.50,
").unwrap_err();
        assert!(err.to_string().starts_with("Row 2:"), "{err}");
        assert!(read_csv("what,ever\n1,2\n").is_err());
    }

    #[test]
    fn json_round_trip_replaces_or_adds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.json");
        let db = sample();
        assert_eq!(export_json(&db, &path).unwrap(), (9, 2));

        let mut replaced = Db::open_in_memory().unwrap();
        replaced.add_category("Gone after replacing", None).unwrap();
        import_json(&mut replaced, &path, true).unwrap();
        assert_eq!(replaced.categories().unwrap(), db.categories().unwrap());
        assert_eq!(replaced.all_entries().unwrap(), db.all_entries().unwrap());

        let mut added = sample();
        assert_eq!(import_json(&mut added, &path, false).unwrap(), (0, 2));
        assert_eq!(added.all_entries().unwrap().len(), 4);
    }

    #[test]
    fn sql_export_escapes_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("budget.sql");
        export_sql(&sample(), &path).unwrap();
        let sql = std::fs::read_to_string(&path).unwrap();
        assert!(sql.contains("`price` DECIMAL(8,2) NOT NULL"));
        assert!(sql.contains("`short` VARCHAR(32) COLLATE utf8mb4_bin NOT NULL UNIQUE"));
        assert_eq!(sql.matches("DEFAULT CHARSET = utf8mb4;").count(), 3);
        assert!(sql.contains("'O''Neill''s \"pub\"', '2026-10-03', -3.00, 30)"), "{sql}");
        assert!(sql.contains("(23, 200.00)"));
        assert_eq!(sql_string("plain"), "'plain'");
        assert_eq!(sql_string(""), "''");
        assert_eq!(sql_string("it's"), "'it''s'");
        assert_eq!(sql_string("a\\b\nc"), "CONCAT('a', CHAR(92 USING utf8mb4), 'b\nc')");
        assert_eq!(sql_string("\\"), "CHAR(92 USING utf8mb4)");
    }

    /// Names that try to break out of their quotes, or that a server could
    /// read differently, come back exactly when the export is loaded, and
    /// nothing in them runs, whatever the server's SQL mode. Loading needs a
    /// test server (see `db::tests::test_server`); the export is checked
    /// without one.
    #[test]
    fn sql_export_loads_hostile_names_exactly() {
        const HOSTILE: [&str; 10] = [
            "x'); DROP TABLE category; --",
            "\\'); DROP TABLE spending; #",
            "a\\\\b \\n \\0 \\Z \\' \\\" end\\",
            "nul \0 byte and ctrl-z \u{1a} here",
            "line\nbreak\r\nand tab\tthere",
            "quotes ' '' \" ` and `` done",
            "look-alikes ʼ ＇ ′ ‘ ’ “ ” 😀",
            "*/ /* comment -- # ; %_",
            "'",
            "\\",
        ];
        let mut db = Db::open_in_memory().unwrap();
        let rows: Vec<(Entry, String)> = HOSTILE
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let date = "2026-10-02".parse().unwrap();
                let category: String = format!("{i}{name}").chars().take(32).collect();
                (Entry { line: 0, name: (*name).into(), date, price: -(i as i64) * 101, bucket: 0 }, category)
            })
            .collect();
        db.add_entries_by_category_name(&rows).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hostile.sql");
        export_sql(&db, &path).unwrap();
        let script = std::fs::read_to_string(&path).unwrap();
        assert!(!script.contains('\0') && !script.contains('\u{1a}'), "bytes that end a file early are written as CHAR()");

        for mode in ["", "NO_BACKSLASH_ESCAPES", "ANSI_QUOTES,NO_BACKSLASH_ESCAPES"] {
            let Some(test) = crate::db::tests::TestDatabase::new() else { return };
            crate::db::run_script(&test.0, &format!("SET SESSION sql_mode = '{mode}';\n{script}"))
                .unwrap_or_else(|e| panic!("{mode:?}: {e:#}"));
            let loaded = Db::connect(&test.0).unwrap();
            assert_eq!(loaded.categories().unwrap(), db.categories().unwrap(), "sql_mode {mode:?}");
            assert_eq!(loaded.all_entries().unwrap(), db.all_entries().unwrap(), "sql_mode {mode:?}");
        }
    }

    #[test]
    fn formats_from_extensions() {
        assert_eq!(Format::of(Path::new("a.CSV")), Some(Format::Csv));
        assert_eq!(Format::of(Path::new("a.json")), Some(Format::Json));
        assert_eq!(Format::of(Path::new("a.sql")), Some(Format::Sql));
        assert_eq!(Format::of(Path::new("a")), None);
    }
}
