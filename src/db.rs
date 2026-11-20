//! SQLite storage. The schema follows `assets/schema/mariadb.sql`, translated to
//! SQLite: `category` and `spending` keep their table and column names, the
//! category ids still start at 22, and the same eight starter categories are
//! added when a new database is created. `price` is stored as whole pence.
//! A `setting` table, added in schema version 2, keeps the choices made on the
//! System Settings tab. `assets/schema/sqlite.sql` is the whole schema in one file,
//! and a test checks that it matches what is built here.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::Path;
use std::time::Duration;

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
-- Matches `AUTO_INCREMENT = 22` in assets/schema/mariadb.sql.
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

const SCHEMA_V2: &str = "
CREATE TABLE IF NOT EXISTS setting (
    name  TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
PRAGMA user_version = 2;
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

/// An entry to be saved, with its category by name, as an imported file has it.
#[derive(Debug, Clone, PartialEq)]
pub struct NewSpending {
    pub date: NaiveDate,
    pub name: String,
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

    /// A database that is lost when the app closes, used when the real one
    /// can't be opened, so the app still works.
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        // A second copy of the app, opened at the same moment, waits its turn
        // instead of failing with "database is locked" and keeping nothing.
        conn.busy_timeout(Duration::from_secs(10))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        // Take the write lock before reading the version, so that two copies
        // opening a new database at once don't both try to create it.
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        log::debug!("database at schema version {version}");
        if version < 1 {
            log::info!("new database: creating schema version 1 and starter categories");
            tx.execute_batch(SCHEMA_V1)?;
        }
        if version < 2 {
            log::info!("updating the database to schema version 2");
            tx.execute_batch(SCHEMA_V2)?;
        }
        tx.commit()?;
        Ok(Self { conn })
    }

    /// A saved setting, if there is one.
    pub fn setting(&self, name: &str) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM setting WHERE name = ?1", params![name], |r| r.get(0))
            .optional()
    }

    /// Saves a setting, or forgets it when `value` is `None`.
    pub fn set_setting(&self, name: &str, value: Option<&str>) -> Result<()> {
        match value {
            Some(v) => self.conn.execute(
                "INSERT INTO setting (name, value) VALUES (?1, ?2)
                 ON CONFLICT (name) DO UPDATE SET value = excluded.value",
                params![name, v],
            ),
            None => self.conn.execute("DELETE FROM setting WHERE name = ?1", params![name]),
        }?;
        Ok(())
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

    #[cfg(test)]
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

    /// Every entry, oldest first.
    pub fn all_spending(&self) -> Result<Vec<Spending>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.line, s.name, s.date, s.price, c.short
             FROM spending s JOIN category c ON c.id = s.bucket
             ORDER BY s.date, s.line",
        )?;
        let rows = stmt.query_map([], row_to_spending)?;
        rows.collect()
    }

    /// Which of `names` aren't categories yet, each once, in the order given.
    /// Names are matched as the database matches them: ignoring the case of
    /// A to Z.
    pub fn missing_categories<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> Result<Vec<String>> {
        let mut known: HashMap<String, i64> = category_ids(&self.categories()?);
        let mut missing = Vec::new();
        for name in names {
            if known.insert(name.to_ascii_lowercase(), 0).is_none() {
                missing.push(name.to_owned());
            }
        }
        Ok(missing)
    }

    /// Adds `categories` that don't exist yet, then `entries`, each with the
    /// category it names, which is added too if need be. Either all of it is
    /// saved or, if anything fails, none of it. Returns the categories added.
    pub fn import(&self, categories: &[String], entries: &[NewSpending]) -> Result<Vec<String>> {
        let tx = self.conn.unchecked_transaction()?;
        let mut ids = category_ids(&self.categories()?);
        let mut added = Vec::new();
        let mut insert = tx.prepare("INSERT INTO spending (name, date, price, bucket) VALUES (?1, ?2, ?3, ?4)")?;
        let wanted = categories.iter().map(String::as_str).chain(entries.iter().map(|e| e.category.as_str()));
        for name in wanted {
            if let Entry::Vacant(slot) = ids.entry(name.to_ascii_lowercase()) {
                tx.execute("INSERT INTO category (short) VALUES (?1)", params![name])?;
                slot.insert(tx.last_insert_rowid());
                added.push(name.to_owned());
            }
        }
        for e in entries {
            insert.execute(params![e.name, iso(e.date), e.pence, ids[&e.category.to_ascii_lowercase()]])?;
        }
        drop(insert);
        tx.commit()?;
        Ok(added)
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

/// Category ids by name, in lower case.
fn category_ids(categories: &[Category]) -> HashMap<String, i64> {
    categories.iter().map(|c| (c.short.to_ascii_lowercase(), c.id)).collect()
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
    fn settings_are_saved_replaced_and_forgotten() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.setting("locale").unwrap(), None);
        db.set_setting("locale", Some("en_GB")).unwrap();
        db.set_setting("locale", Some("de_DE")).unwrap();
        assert_eq!(db.setting("locale").unwrap().as_deref(), Some("de_DE"));
        db.set_setting("locale", None).unwrap();
        assert_eq!(db.setting("locale").unwrap(), None);
    }

    #[test]
    fn a_version_1_database_is_updated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(SCHEMA_V1).unwrap();
        drop(conn);
        let db = Db::open(&path).unwrap();
        db.set_setting("appearance", Some("high_contrast")).unwrap();
        assert_eq!(db.categories().unwrap().len(), 8, "not seeded twice");
    }

    /// Everything that defines a database: its tables, indexes, starting
    /// categories, next category id and version.
    fn shape(conn: &Connection) -> Vec<String> {
        let mut rows: Vec<String> = Vec::new();
        let mut stmt = conn.prepare("SELECT type, name, sql FROM sqlite_master ORDER BY type, name").unwrap();
        rows.extend(
            stmt.query_map([], |r| {
                // Line endings don't matter, and Windows checkouts may have CRLF.
                let sql = r.get::<_, Option<String>>(2)?.unwrap_or_default().replace("\r\n", "\n");
                Ok(format!("{} {} {sql}", r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .unwrap()
            .map(|r| r.unwrap()),
        );
        let mut stmt = conn.prepare("SELECT id, short FROM category ORDER BY id").unwrap();
        rows.extend(stmt.query_map([], |r| Ok(format!("{} {}", r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).unwrap().map(|r| r.unwrap()));
        rows.push(conn.query_row("SELECT seq FROM sqlite_sequence WHERE name = 'category'", [], |r| r.get::<_, i64>(0)).unwrap().to_string());
        rows.push(conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap().to_string());
        rows
    }

    #[test]
    fn the_sample_schema_matches_the_real_one() {
        let sample = Connection::open_in_memory().unwrap();
        sample.execute_batch(include_str!("../assets/schema/sqlite.sql")).unwrap();
        let real = Db::open_in_memory().unwrap();
        assert_eq!(shape(&sample), shape(&real.conn));
    }

    fn new(date: &str, name: &str, pence: i64, category: &str) -> NewSpending {
        NewSpending { date: d(date), name: name.to_owned(), pence, category: category.to_owned() }
    }

    #[test]
    fn importing_adds_entries_and_missing_categories() {
        let db = Db::open_in_memory().unwrap();
        let entries = [
            new("2026-10-01", "Tea", 250, "food and drink"),
            new("2026-11-02", "Lead", 1000, "Pets"),
            new("2026-11-03", "Food", 800, "PETS"),
        ];
        let wanted = ["Gifts".to_owned()];
        let missing = db.missing_categories(wanted.iter().map(String::as_str).chain(entries.iter().map(|e| e.category.as_str())));
        assert_eq!(missing.unwrap(), ["Gifts", "Pets"]);
        assert_eq!(db.import(&wanted, &entries).unwrap(), ["Gifts", "Pets"]);
        let all = db.all_spending().unwrap();
        assert_eq!(all.iter().map(|s| s.category.as_str()).collect::<Vec<_>>(), ["Food and Drink", "Pets", "Pets"]);
        assert_eq!(db.categories().unwrap().len(), 10);
    }

    #[test]
    fn a_failed_import_saves_nothing() {
        let db = Db::open_in_memory().unwrap();
        // The second name is too long for the database.
        let entries = [new("2026-10-01", "Tea", 250, "Pets"), new("2026-10-02", &"x".repeat(161), 1, "Pets")];
        assert!(db.import(&[], &entries).is_err());
        assert!(db.all_spending().unwrap().is_empty());
        assert_eq!(db.categories().unwrap().len(), 8, "the new category went too");
    }

    #[test]
    fn unknown_category_rejected() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.add_spending("X", d("2026-11-01"), 100, 999).is_err());
    }
}
