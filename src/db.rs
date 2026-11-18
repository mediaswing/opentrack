//! SQLite storage. The schema follows `MariaDB Schema.sql`, translated to
//! SQLite: `category` and `spending` keep their table and column names, the
//! category ids still start at 22, and the same eight starter categories are
//! added when a new database is created. `price` is stored as whole pence.

use std::path::Path;

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension, Result, TransactionBehavior};

const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS category (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    short TEXT NOT NULL UNIQUE COLLATE NOCASE CHECK (length(short) BETWEEN 1 AND 32)
);
CREATE TABLE IF NOT EXISTS spending (
    line   INTEGER PRIMARY KEY AUTOINCREMENT,
    name   TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    date   TEXT    NOT NULL CHECK (date(date) IS date),
    price  INTEGER NOT NULL CHECK (abs(price) <= 99999999),
    bucket INTEGER NOT NULL REFERENCES category (id)
);
CREATE INDEX IF NOT EXISTS spending_date ON spending (date);
-- Matches `AUTO_INCREMENT = 22` in the MariaDB schema.
INSERT INTO sqlite_sequence (name, seq) VALUES ('category', 21);
INSERT INTO category (short) VALUES
    ('Entertainment'),
    ('Food and Drink'),
    ('Utilities and Bills'),
    ('Web Services'),
    ('Transport'),
    ('Accommodation'),
    ('Healthcare'),
    ('Social Events');
PRAGMA user_version = 1;
";

#[derive(Debug, Clone)]
pub struct Category {
    pub id: i64,
    pub short: String,
}

#[derive(Debug, Clone)]
pub struct Spending {
    pub line: i64,
    pub name: String,
    pub date: NaiveDate,
    pub pence: i64,
    pub category: String,
}

#[derive(Debug, Clone)]
pub struct CategoryTotal {
    pub category: String,
    pub pence: i64,
    pub count: i64,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        // Take the write lock before reading the version, so that two copies
        // opening a new database at once don't both try to create it.
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version < 1 {
            log::info!("new database: creating schema version 1 and starter categories");
            tx.execute_batch(SCHEMA_V1)?;
        } else {
            log::debug!("existing database at schema version {version}");
        }
        tx.commit()?;
        Ok(Self { conn })
    }

    pub fn categories(&self) -> Result<Vec<Category>> {
        let mut stmt = self.conn.prepare("SELECT id, short FROM category ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok(Category { id: r.get(0)?, short: r.get(1)? }))?;
        rows.collect()
    }

    pub fn add_category(&self, short: &str) -> Result<i64> {
        self.conn.execute("INSERT INTO category (short) VALUES (?1)", params![short])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn add_spending(&self, name: &str, date: NaiveDate, pence: i64, bucket: i64) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO spending (name, date, price, bucket) VALUES (?1, ?2, ?3, ?4)",
            params![name, date.format("%Y-%m-%d").to_string(), pence, bucket],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_spending(&self, line: i64) -> Result<Option<Spending>> {
        self.conn
            .query_row(
                "SELECT s.line, s.name, s.date, s.price, c.short
                 FROM spending s JOIN category c ON c.id = s.bucket
                 WHERE s.line = ?1",
                params![line],
                row_to_spending,
            )
            .optional()
    }

    pub fn delete_spending(&self, line: i64) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM spending WHERE line = ?1", params![line])? > 0)
    }

    /// Spending from `from` (inclusive) up to `to` (exclusive), oldest first.
    pub fn spending_between(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<Spending>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.line, s.name, s.date, s.price, c.short
             FROM spending s JOIN category c ON c.id = s.bucket
             WHERE s.date >= ?1 AND s.date < ?2
             ORDER BY s.date, s.line",
        )?;
        let rows = stmt.query_map(params![iso(from), iso(to)], row_to_spending)?;
        rows.collect()
    }

    /// Totals per category between two dates, largest first.
    pub fn totals_between(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<CategoryTotal>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.short, SUM(s.price), COUNT(*)
             FROM spending s JOIN category c ON c.id = s.bucket
             WHERE s.date >= ?1 AND s.date < ?2
             GROUP BY c.id
             ORDER BY SUM(s.price) DESC, c.short",
        )?;
        let rows = stmt.query_map(params![iso(from), iso(to)], |r| {
            Ok(CategoryTotal { category: r.get(0)?, pence: r.get(1)?, count: r.get(2)? })
        })?;
        rows.collect()
    }
}

fn iso(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn row_to_spending(r: &rusqlite::Row) -> Result<Spending> {
    let date: String = r.get(2)?;
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(Spending { line: r.get(0)?, name: r.get(1)?, date, pence: r.get(3)?, category: r.get(4)? })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn seeds_categories_from_22() {
        let db = Db::open_in_memory().unwrap();
        let cats = db.categories().unwrap();
        assert_eq!(cats.len(), 8);
        assert_eq!(cats[0].id, 22);
        assert_eq!(cats[0].short, "Entertainment");
        assert_eq!(cats[7].id, 29);
        assert_eq!(db.add_category("Gifts").unwrap(), 30);
    }

    #[test]
    fn duplicate_category_rejected_case_insensitively() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.add_category("transport").is_err());
    }

    #[test]
    fn add_list_total_delete() {
        let db = Db::open_in_memory().unwrap();
        let a = db.add_spending("Cinema", d("2026-11-03"), 1250, 22).unwrap();
        db.add_spending("Pizza", d("2026-11-05"), 900, 23).unwrap();
        db.add_spending("Bus", d("2026-12-01"), 200, 26).unwrap();

        let nov = db.spending_between(d("2026-11-01"), d("2026-12-01")).unwrap();
        assert_eq!(nov.len(), 2);
        assert_eq!(nov[0].category, "Entertainment");

        let totals = db.totals_between(d("2026-11-01"), d("2026-12-01")).unwrap();
        assert_eq!(totals[0].category, "Entertainment");
        assert_eq!(totals.iter().map(|t| t.pence).sum::<i64>(), 2150);

        assert!(db.delete_spending(a).unwrap());
        assert!(db.get_spending(a).unwrap().is_none());
        assert!(!db.delete_spending(a).unwrap());
    }

    #[test]
    fn reopening_keeps_data_and_does_not_reseed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        Db::open(&path).unwrap().add_spending("Tea", d("2026-11-01"), 250, 23).unwrap();
        let db = Db::open(&path).unwrap();
        assert_eq!(db.categories().unwrap().len(), 8);
        assert_eq!(db.spending_between(d("2026-11-01"), d("2026-12-01")).unwrap().len(), 1);
    }

    #[test]
    fn unknown_category_rejected() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.add_spending("X", d("2026-11-01"), 100, 999).is_err());
    }
}
