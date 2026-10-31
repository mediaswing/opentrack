//! The built-in database: SQLite, in a file in the app's data folder.
//!
//! Differences from the MariaDB schema:
//! * `price` is whole pence as an INTEGER, not `DECIMAL(8,2)`, so sums are
//!   exact; it holds the same range.
//! * `date` is ISO 8601 text (YYYY-MM-DD), checked by SQLite's `date()`.
//! * The VARCHAR lengths are CHECK constraints.

use super::{Category, Days, Entry, FIRST_CATEGORY_ID, STARTING_CATEGORIES, Store, find_category};
use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use rusqlite::{Connection, params};
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

const ENTRY_COLUMNS: &str = "line, name, date, price, bucket";

pub struct Sqlite {
    conn: Connection,
}

impl Sqlite {
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
        Ok(Sqlite { conn })
    }
}

fn set_budget(conn: &Connection, id: i64, budget: Option<i64>) -> Result<()> {
    match budget {
        Some(pence) => conn.execute(
            "INSERT INTO budget (bucket, monthly) VALUES (?1, ?2)
             ON CONFLICT (bucket) DO UPDATE SET monthly = excluded.monthly",
            params![id, pence],
        )?,
        None => conn.execute("DELETE FROM budget WHERE bucket = ?1", [id])?,
    };
    Ok(())
}

fn categories(conn: &Connection) -> Result<Vec<Category>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.short, b.monthly FROM category c LEFT JOIN budget b ON b.bucket = c.id
         ORDER BY c.short COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| Ok(Category { id: r.get(0)?, short: r.get(1)?, budget: r.get(2)? }))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn entry_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Entry> {
    let date: String = r.get(2)?;
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;
    Ok(Entry { line: r.get(0)?, name: r.get(1)?, date, price: r.get(3)?, bucket: r.get(4)? })
}

/// The first and last days as text, which compares as dates do.
fn day_params((first, last): Days) -> (String, String) {
    (first.to_string(), last.to_string())
}

impl Store for Sqlite {
    fn categories(&mut self) -> Result<Vec<Category>> {
        categories(&self.conn)
    }

    fn add_category(&mut self, short: &str, budget: Option<i64>) -> Result<i64> {
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO category (short) VALUES (?1)", [short])?;
        let id = tx.last_insert_rowid();
        set_budget(&tx, id, budget)?;
        tx.commit()?;
        Ok(id)
    }

    fn update_category(&mut self, id: i64, short: &str, budget: Option<i64>) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE category SET short = ?1 WHERE id = ?2", params![short, id])?;
        set_budget(&tx, id, budget)?;
        tx.commit()?;
        Ok(())
    }

    fn entry_counts(&mut self) -> Result<Vec<(i64, i64)>> {
        let mut stmt = self.conn.prepare("SELECT bucket, count(*) FROM spending GROUP BY bucket")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn delete_category(&mut self, id: i64, move_to: Option<i64>) -> Result<i64> {
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
        Ok(count)
    }

    fn entries(&mut self, days: Option<Days>, bucket: Option<i64>) -> Result<Vec<Entry>> {
        let (first, last) = days.map_or((String::new(), "9999-99-99".to_owned()), day_params);
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ENTRY_COLUMNS} FROM spending
             WHERE date >= ?1 AND date <= ?2 AND (?3 IS NULL OR bucket = ?3)
             ORDER BY date DESC, line DESC"
        ))?;
        let rows = stmt.query_map(params![first, last, bucket], entry_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn all_entries(&mut self) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {ENTRY_COLUMNS} FROM spending ORDER BY line"))?;
        let rows = stmt.query_map([], entry_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn add_entry(&mut self, e: &Entry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO spending (name, date, price, bucket) VALUES (?1, ?2, ?3, ?4)",
            params![e.name, e.date.to_string(), e.price, e.bucket],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn update_entry(&mut self, e: &Entry) -> Result<bool> {
        let changed = self.conn.execute(
            "UPDATE spending SET name = ?1, date = ?2, price = ?3, bucket = ?4 WHERE line = ?5",
            params![e.name, e.date.to_string(), e.price, e.bucket, e.line],
        )?;
        Ok(changed > 0)
    }

    fn delete_entry(&mut self, line: i64) -> Result<()> {
        self.conn.execute("DELETE FROM spending WHERE line = ?1", [line])?;
        Ok(())
    }

    fn entry_count_between(&mut self, days: Days) -> Result<i64> {
        let (first, last) = day_params(days);
        Ok(self.conn.query_row("SELECT count(*) FROM spending WHERE date >= ?1 AND date <= ?2", params![first, last], |r| {
            r.get(0)
        })?)
    }

    fn totals(&mut self, days: Days) -> Result<Vec<(i64, i64)>> {
        let (first, last) = day_params(days);
        let mut stmt =
            self.conn.prepare("SELECT bucket, sum(price) FROM spending WHERE date >= ?1 AND date <= ?2 GROUP BY bucket")?;
        let rows = stmt.query_map(params![first, last], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn month_totals(&mut self, days: Days) -> Result<Vec<(String, i64)>> {
        let (first, last) = day_params(days);
        let mut stmt = self.conn.prepare(
            "SELECT substr(date, 1, 7), sum(price) FROM spending WHERE date >= ?1 AND date <= ?2
             GROUP BY substr(date, 1, 7)",
        )?;
        let rows = stmt.query_map(params![first, last], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn replace_all(&mut self, categories: &[Category], entries: &[Entry]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute_batch("DELETE FROM spending; DELETE FROM budget; DELETE FROM category;")?;
        for c in categories {
            tx.execute("INSERT INTO category (id, short) VALUES (?1, ?2)", params![c.id, c.short])
                .with_context(|| t!("context-category", id = c.id, name = &c.short))?;
            if let Some(pence) = c.budget {
                tx.execute("INSERT INTO budget (bucket, monthly) VALUES (?1, ?2)", params![c.id, pence])
                    .with_context(|| t!("context-budget", name = &c.short))?;
            }
        }
        for e in entries {
            tx.execute(
                "INSERT INTO spending (line, name, date, price, bucket) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![e.line, e.name, e.date.to_string(), e.price, e.bucket],
            )
            .with_context(|| t!("context-entry", line = e.line))?;
        }
        tx.commit()?;
        Ok(())
    }

    fn add_entries_by_category_name(&mut self, entries: &[(Entry, String)]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        let mut known = categories(&tx)?;
        let mut created = 0;
        for (i, (e, short)) in entries.iter().enumerate() {
            let bucket = match find_category(&known, short) {
                Some(id) => id,
                None => {
                    tx.execute("INSERT INTO category (short) VALUES (?1)", [short])?;
                    created += 1;
                    let id = tx.last_insert_rowid();
                    known.push(Category { id, short: short.clone(), budget: None });
                    id
                }
            };
            tx.execute(
                "INSERT INTO spending (name, date, price, bucket) VALUES (?1, ?2, ?3, ?4)",
                params![e.name, e.date.to_string(), e.price, bucket],
            )
            .with_context(|| t!("context-entry", line = i + 1))?;
        }
        tx.commit()?;
        Ok(created)
    }

    fn back_up(&mut self, path: &Path) -> Result<()> {
        // SQLite takes the name as text, so a path that isn't would be
        // changed, and the copy written somewhere else.
        let name = path.to_str().with_context(|| t!("error-path-not-text", path = path.display().to_string()))?;
        self.conn.execute("VACUUM INTO ?1", [name])?;
        Ok(())
    }
}
