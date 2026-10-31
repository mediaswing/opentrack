//! The budget database: SQLite, kept in the app's data folder.
//!
//! The tables follow the MariaDB schema the app was designed from:
//! `category` (`id`, `short`) and `spending` (`line`, `name`, `date`,
//! `price`, `bucket`), with category ids starting at 22 and the same eight
//! starting categories, named in the interface's language. Differences, for
//! SQLite:
//! * `price` is whole pence as an INTEGER, not `DECIMAL(8,2)`, so sums are
//!   exact; it holds the same range.
//! * `date` is ISO 8601 text (YYYY-MM-DD), checked by SQLite's `date()`.
//! * The VARCHAR lengths are CHECK constraints.
//! * A `budget` table adds an optional monthly budget for each category.

use crate::money::{MAX_PENCE, Month, Period};
use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

/// The schema version this build writes, in `PRAGMA user_version`.
const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS category (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    short TEXT NOT NULL UNIQUE COLLATE NOCASE CHECK (length(short) BETWEEN 1 AND 32)
);

CREATE TABLE IF NOT EXISTS spending (
    line INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    date TEXT NOT NULL CHECK (date(date) IS date),
    price INTEGER NOT NULL CHECK (price BETWEEN -99999999 AND 99999999),
    bucket INTEGER NOT NULL REFERENCES category (id)
);

CREATE INDEX IF NOT EXISTS spending_date ON spending (date);
CREATE INDEX IF NOT EXISTS spending_bucket ON spending (bucket);

CREATE TABLE IF NOT EXISTS budget (
    bucket INTEGER PRIMARY KEY REFERENCES category (id) ON DELETE CASCADE,
    monthly INTEGER NOT NULL CHECK (monthly BETWEEN 0 AND 99999999)
);
";

/// The schema's `ALTER TABLE category AUTO_INCREMENT = 22`.
const FIRST_CATEGORY_ID: i64 = 22;

/// The messages naming the starting categories, which are added in the
/// interface's language when the database is made.
pub(crate) const STARTING_CATEGORIES: [&str; 8] = [
    "start-entertainment",
    "start-food-and-drink",
    "start-utilities-and-bills",
    "start-web-services",
    "start-transport",
    "start-accommodation",
    "start-healthcare",
    "start-social-events",
];

pub const MAX_CATEGORY_CHARS: usize = 32;
pub const MAX_NAME_CHARS: usize = 160;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: i64,
    pub short: String,
    /// The monthly budget in pence, if one is set.
    pub budget: Option<i64>,
}

/// A line of spending.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub line: i64,
    pub name: String,
    pub date: NaiveDate,
    /// In pence. Negative for a refund.
    pub price: i64,
    /// The category's id.
    pub bucket: i64,
}

/// Which entries the Spending tab lists.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    /// Only entries on these dates.
    pub period: Option<Period>,
    /// Only entries whose name contains this, ignoring case.
    pub text: String,
    pub bucket: Option<i64>,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            crate::paths::create_private_dir(dir).with_context(|| t!("error-create-folder", path = dir.display().to_string()))?;
        }
        let conn = Connection::open(path).with_context(|| t!("error-open-file", path = path.display().to_string()))?;
        Self::setup(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn setup(mut conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        // With the app open twice on one database, a write waits for the
        // other copy's to finish rather than failing at once.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            bail!(t!("db-newer", version = version));
        }
        if version < 1 {
            log::info!("creating the database schema");
            // The starting categories go in the same transaction as the
            // schema version, so an interrupted first run is redone in full.
            let tx = conn.transaction()?;
            tx.execute_batch(&format!(
                "{SCHEMA}
                 INSERT INTO sqlite_sequence (name, seq) SELECT 'category', {}
                     WHERE NOT EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'category');",
                FIRST_CATEGORY_ID - 1
            ))?;
            let empty: bool = tx.query_row("SELECT NOT EXISTS (SELECT 1 FROM category)", [], |r| r.get(0))?;
            if empty {
                for id in STARTING_CATEGORIES {
                    tx.execute("INSERT INTO category (short) VALUES (?1)", [crate::i18n::message(id, None)])?;
                }
            }
            tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            tx.commit()?;
        }
        Ok(Db { conn })
    }

    // ----- categories ----------------------------------------------------

    /// Every category, in alphabetical order.
    pub fn categories(&self) -> Result<Vec<Category>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.short, b.monthly FROM category c LEFT JOIN budget b ON b.bucket = c.id
             ORDER BY c.short COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| Ok(Category { id: r.get(0)?, short: r.get(1)?, budget: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn category_by_name(&self, short: &str) -> Result<Option<i64>> {
        Ok(self.conn.query_row("SELECT id FROM category WHERE short = ?1", [short.trim()], |r| r.get(0)).optional()?)
    }

    pub fn add_category(&self, short: &str, budget: Option<i64>) -> Result<i64> {
        let short = check_category_name(short)?;
        if self.category_by_name(&short)?.is_some() {
            bail!(t!("category-exists", name = &short));
        }
        self.conn.execute("INSERT INTO category (short) VALUES (?1)", [&short])?;
        let id = self.conn.last_insert_rowid();
        self.set_budget(id, budget)?;
        log::info!("added category {id}");
        Ok(id)
    }

    pub fn update_category(&self, id: i64, short: &str, budget: Option<i64>) -> Result<()> {
        let short = check_category_name(short)?;
        if self.category_by_name(&short)?.is_some_and(|other| other != id) {
            bail!(t!("category-exists", name = &short));
        }
        self.conn.execute("UPDATE category SET short = ?1 WHERE id = ?2", params![short, id])?;
        self.set_budget(id, budget)?;
        log::info!("updated category {id}");
        Ok(())
    }

    fn set_budget(&self, id: i64, budget: Option<i64>) -> Result<()> {
        match budget {
            Some(pence) if !(0..=MAX_PENCE).contains(&pence) => bail!(t!("budget-range")),
            Some(pence) => self.conn.execute(
                "INSERT INTO budget (bucket, monthly) VALUES (?1, ?2)
                 ON CONFLICT (bucket) DO UPDATE SET monthly = excluded.monthly",
                params![id, pence],
            )?,
            None => self.conn.execute("DELETE FROM budget WHERE bucket = ?1", [id])?,
        };
        Ok(())
    }

    /// How many entries are in a category.
    pub fn entry_count(&self, bucket: i64) -> Result<i64> {
        Ok(self.conn.query_row("SELECT count(*) FROM spending WHERE bucket = ?1", [bucket], |r| r.get(0))?)
    }

    /// How many entries each category has, leaving out those with none.
    pub fn entry_counts(&self) -> Result<Vec<(i64, i64)>> {
        let mut stmt = self.conn.prepare("SELECT bucket, count(*) FROM spending GROUP BY bucket")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Deletes a category. Its entries move to `move_to` first; without one,
    /// a category that has entries can't be deleted.
    pub fn delete_category(&mut self, id: i64, move_to: Option<i64>) -> Result<()> {
        let tx = self.conn.transaction()?;
        let count: i64 = tx.query_row("SELECT count(*) FROM spending WHERE bucket = ?1", [id], |r| r.get(0))?;
        if count > 0 {
            match move_to {
                Some(to) if to != id => {
                    tx.execute("UPDATE spending SET bucket = ?1 WHERE bucket = ?2", params![to, id])?;
                }
                _ => bail!(t!("db-move-entries-first", count = count)),
            }
        }
        tx.execute("DELETE FROM category WHERE id = ?1", [id])?;
        tx.commit()?;
        log::info!("deleted category {id}, moving {count} entries to {move_to:?}");
        Ok(())
    }

    // ----- spending ------------------------------------------------------

    /// The entries matching `filter`, newest first.
    pub fn entries(&self, filter: &Filter) -> Result<Vec<Entry>> {
        let (from, to) = match filter.period {
            Some(p) => (p.first_day().to_string(), p.end().to_string()),
            None => (String::new(), "9999-99-99".to_owned()),
        };
        let mut stmt = self.conn.prepare(
            "SELECT line, name, date, price, bucket FROM spending
             WHERE date >= ?1 AND date < ?2
               AND (?3 IS NULL OR bucket = ?3)
             ORDER BY date DESC, line DESC",
        )?;
        let rows = stmt.query_map(params![from, to, filter.bucket], entry_from_row)?;
        let entries: Vec<Entry> = rows.collect::<rusqlite::Result<_>>()?;
        // Matched here rather than in SQL, whose lower() only knows ASCII.
        let text = filter.text.trim().to_lowercase();
        Ok(entries.into_iter().filter(|e| text.is_empty() || e.name.to_lowercase().contains(&text)).collect())
    }

    pub fn all_entries(&self) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare("SELECT line, name, date, price, bucket FROM spending ORDER BY line")?;
        let rows = stmt.query_map([], entry_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Adds an entry, ignoring its `line`, and returns the line it was given.
    pub fn add_entry(&self, entry: &Entry) -> Result<i64> {
        let name = check_entry(entry)?;
        self.conn.execute(
            "INSERT INTO spending (name, date, price, bucket) VALUES (?1, ?2, ?3, ?4)",
            params![name, entry.date.to_string(), entry.price, entry.bucket],
        )?;
        let line = self.conn.last_insert_rowid();
        log::info!("added entry {line}");
        Ok(line)
    }

    pub fn update_entry(&self, entry: &Entry) -> Result<()> {
        let name = check_entry(entry)?;
        let changed = self.conn.execute(
            "UPDATE spending SET name = ?1, date = ?2, price = ?3, bucket = ?4 WHERE line = ?5",
            params![name, entry.date.to_string(), entry.price, entry.bucket, entry.line],
        )?;
        if changed == 0 {
            bail!(t!("entry-gone", line = entry.line));
        }
        log::info!("updated entry {}", entry.line);
        Ok(())
    }

    pub fn delete_entry(&self, line: i64) -> Result<()> {
        self.conn.execute("DELETE FROM spending WHERE line = ?1", [line])?;
        log::info!("deleted entry {line}");
        Ok(())
    }

    // ----- summaries -----------------------------------------------------

    /// How many entries are in a period.
    pub fn period_entry_count(&self, period: Period) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM spending WHERE date >= ?1 AND date < ?2",
            params![period.first_day().to_string(), period.end().to_string()],
            |r| r.get(0),
        )?)
    }

    /// Each category's total for a period, keyed by category id. Categories
    /// with nothing spent are left out.
    pub fn totals(&self, period: Period) -> Result<Vec<(i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT bucket, sum(price) FROM spending WHERE date >= ?1 AND date < ?2 GROUP BY bucket",
        )?;
        let rows = stmt.query_map(params![period.first_day().to_string(), period.end().to_string()], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The total spent in each month with a day in `period`, counting only
    /// the days in it, oldest first, including months with nothing spent.
    pub fn monthly_totals(&self, period: Period) -> Result<Vec<(Month, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT substr(date, 1, 7), sum(price) FROM spending WHERE date >= ?1 AND date < ?2
             GROUP BY substr(date, 1, 7)",
        )?;
        let rows: Vec<(String, i64)> = stmt
            .query_map(params![period.first_day().to_string(), period.end().to_string()], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(period
            .months()
            .into_iter()
            .map(|month| {
                let key = month.key();
                (month, rows.iter().find(|(k, _)| *k == key).map_or(0, |(_, total)| *total))
            })
            .collect())
    }

    // ----- whole database ------------------------------------------------

    /// Replaces everything with `categories` and `entries`, keeping their
    /// ids, in one transaction: if anything is wrong, nothing changes.
    pub fn replace_all(&mut self, categories: &[Category], entries: &[Entry]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute_batch("DELETE FROM spending; DELETE FROM budget; DELETE FROM category;")?;
        for c in categories {
            let short = check_category_name(&c.short)?;
            tx.execute("INSERT INTO category (id, short) VALUES (?1, ?2)", params![c.id, short])
                .with_context(|| t!("context-category", id = c.id, name = &short))?;
            if let Some(pence) = c.budget {
                tx.execute("INSERT INTO budget (bucket, monthly) VALUES (?1, ?2)", params![c.id, pence])
                    .with_context(|| t!("context-budget", name = &short))?;
            }
        }
        for e in entries {
            let name = check_entry(e)?;
            tx.execute(
                "INSERT INTO spending (line, name, date, price, bucket) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![e.line, name, e.date.to_string(), e.price, e.bucket],
            )
            .with_context(|| t!("context-entry", line = e.line))?;
        }
        tx.commit()?;
        log::info!("replaced the database with {} categories and {} entries", categories.len(), entries.len());
        Ok(())
    }

    /// Adds `entries`, whose `bucket` is a category name looked up or
    /// created, in one transaction. Returns how many categories were new.
    pub fn add_entries_by_category_name(&mut self, entries: &[(Entry, String)]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut created = 0;
        for (i, (entry, category)) in entries.iter().enumerate() {
            let short = check_category_name(category)?;
            let existing: Option<i64> =
                tx.query_row("SELECT id FROM category WHERE short = ?1", [&short], |r| r.get(0)).optional()?;
            let bucket = match existing {
                Some(id) => id,
                None => {
                    tx.execute("INSERT INTO category (short) VALUES (?1)", [&short])?;
                    created += 1;
                    tx.last_insert_rowid()
                }
            };
            let name = check_entry(entry).with_context(|| t!("context-entry", line = i + 1))?;
            tx.execute(
                "INSERT INTO spending (name, date, price, bucket) VALUES (?1, ?2, ?3, ?4)",
                params![name, entry.date.to_string(), entry.price, bucket],
            )?;
        }
        tx.commit()?;
        log::info!("added {} entries and {created} categories", entries.len());
        Ok(created)
    }

    /// Writes a copy of the whole database to `path`, which must not exist.
    pub fn back_up(&self, path: &Path) -> Result<()> {
        self.conn.execute("VACUUM INTO ?1", [path.to_string_lossy()])?;
        log::info!("backed up the database");
        Ok(())
    }
}

fn entry_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    let date: String = r.get(2)?;
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;
    Ok(Entry { line: r.get(0)?, name: r.get(1)?, date, price: r.get(3)?, bucket: r.get(4)? })
}

/// A category name, trimmed, or why it can't be used.
pub fn check_category_name(short: &str) -> Result<String> {
    let short = short.trim();
    if short.is_empty() {
        bail!(t!("category-name-missing"));
    }
    if short.chars().count() > MAX_CATEGORY_CHARS {
        bail!(t!("category-name-too-long", max = MAX_CATEGORY_CHARS));
    }
    Ok(short.to_owned())
}

/// An entry's name, trimmed, after checking the entry fits the schema.
fn check_entry(entry: &Entry) -> Result<String> {
    let name = entry.name.trim();
    if name.is_empty() {
        bail!(t!("entry-name-missing"));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        bail!(t!("entry-name-too-long", max = MAX_NAME_CHARS));
    }
    if entry.price.abs() > MAX_PENCE {
        bail!(t!("entry-amount-too-large"));
    }
    Ok(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, date: &str, price: i64, bucket: i64) -> Entry {
        Entry { line: 0, name: name.into(), date: date.parse().unwrap(), price, bucket }
    }

    #[test]
    fn starts_with_the_schema_categories() {
        let db = Db::open_in_memory().unwrap();
        let categories = db.categories().unwrap();
        assert_eq!(categories.len(), 8);
        let ids: Vec<i64> = {
            let mut ids: Vec<i64> = categories.iter().map(|c| c.id).collect();
            ids.sort();
            ids
        };
        assert_eq!(ids, (22..30).collect::<Vec<_>>());
        assert_eq!(db.category_by_name("food and drink").unwrap(), Some(23));
        assert_eq!(db.add_category("Pets", None).unwrap(), 30);
    }

    #[test]
    fn reopening_keeps_data_and_does_not_reseed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("budget.db");
        {
            let mut db = Db::open(&path).unwrap();
            db.delete_category(22, None).unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(db.categories().unwrap().len(), 7);
    }

    #[test]
    fn entries_filter_and_sum() {
        let db = Db::open_in_memory().unwrap();
        db.add_entry(&entry("Cinema", "2026-10-02", 1200, 22)).unwrap();
        db.add_entry(&entry("Lunch", "2026-10-15", 850, 23)).unwrap();
        db.add_entry(&entry("Dinner", "2026-10-31", 2400, 23)).unwrap();
        db.add_entry(&entry("Brunch", "2026-11-01", 1000, 23)).unwrap();
        let october = Period::Month(Month { year: 2026, month: 10 });

        let all = db.entries(&Filter { period: Some(october), ..Filter::default() }).unwrap();
        assert_eq!(all.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Dinner", "Lunch", "Cinema"]);
        let found = db.entries(&Filter { text: "NCH".into(), ..Filter::default() }).unwrap();
        assert_eq!(found.len(), 2);
        let food = db.entries(&Filter { period: Some(october), bucket: Some(23), ..Filter::default() }).unwrap();
        assert_eq!(food.len(), 2);

        assert_eq!(db.period_entry_count(october).unwrap(), 3);
        let mut totals = db.totals(october).unwrap();
        totals.sort();
        assert_eq!(totals, [(22, 1200), (23, 3250)]);
        let day = |d| NaiveDate::from_ymd_opt(2026, 10, d).unwrap();
        let three_months = Period::Days { first: day(1) - chrono::Months::new(1), last: day(1) + chrono::Months::new(1) };
        let months = db.monthly_totals(three_months).unwrap();
        assert_eq!(months.iter().map(|(_, t)| *t).collect::<Vec<_>>(), [0, 4450, 1000]);
        let part = Period::Days { first: day(10), last: day(31) };
        assert_eq!(db.period_entry_count(part).unwrap(), 2);
        assert_eq!(db.monthly_totals(part).unwrap(), [(Month { year: 2026, month: 10 }, 3250)]);
    }

    #[test]
    fn finding_ignores_case_beyond_ascii() {
        let db = Db::open_in_memory().unwrap();
        db.add_entry(&entry("Café Élan", "2026-10-03", 300, 23)).unwrap();
        let found = db.entries(&Filter { text: "CAFÉ é".into(), ..Filter::default() }).unwrap();
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn checks_entries_against_the_schema() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.add_entry(&entry("  ", "2026-10-02", 1, 22)).is_err());
        assert!(db.add_entry(&entry(&"x".repeat(161), "2026-10-02", 1, 22)).is_err());
        assert!(db.add_entry(&entry("Too much", "2026-10-02", MAX_PENCE + 1, 22)).is_err());
        assert!(db.add_entry(&entry("No such category", "2026-10-02", 1, 999)).is_err());
        assert!(db.add_category("Entertainment", None).is_err(), "names are unique, ignoring case");
        assert!(db.add_category(&"x".repeat(33), None).is_err());
    }

    #[test]
    fn deleting_a_used_category_moves_its_entries() {
        let mut db = Db::open_in_memory().unwrap();
        db.add_entry(&entry("Bus", "2026-10-02", 250, 26)).unwrap();
        assert!(db.delete_category(26, None).is_err());
        assert!(db.delete_category(26, Some(26)).is_err());
        db.delete_category(26, Some(22)).unwrap();
        assert_eq!(db.entry_count(22).unwrap(), 1);
        assert_eq!(db.entry_counts().unwrap(), [(22, 1)]);
        assert_eq!(db.category_by_name("Transport").unwrap(), None);
    }

    #[test]
    fn budgets() {
        let db = Db::open_in_memory().unwrap();
        db.update_category(23, "Food", Some(20_000)).unwrap();
        let food = db.categories().unwrap().into_iter().find(|c| c.id == 23).unwrap();
        assert_eq!((food.short.as_str(), food.budget), ("Food", Some(20_000)));
        db.update_category(23, "Food", None).unwrap();
        assert_eq!(db.categories().unwrap().into_iter().find(|c| c.id == 23).unwrap().budget, None);
        assert!(db.update_category(23, "transport", None).is_err());
    }

    #[test]
    fn replace_all_is_all_or_nothing() {
        let mut db = Db::open_in_memory().unwrap();
        let categories = vec![Category { id: 5, short: "Only".into(), budget: Some(100) }];
        let bad = vec![entry("Fine", "2026-01-01", 1, 5), entry("Wrong category", "2026-01-01", 1, 6)];
        assert!(db.replace_all(&categories, &bad).is_err());
        assert_eq!(db.categories().unwrap().len(), 8, "nothing changed");
        db.replace_all(&categories, &bad[..1]).unwrap();
        assert_eq!(db.categories().unwrap(), categories);
        assert_eq!(db.all_entries().unwrap().len(), 1);
    }
}
