use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, params};

use super::{
    Category, ImportRow, Spending, Store, ensure_unique_category, import_categories, parse_date, split_schema,
    validate_category, validate_spending,
};
use crate::money;

const SCHEMA: &str = include_str!("../../schema/sqlite.sql");

pub struct SqliteStore {
    conn: Connection,
    path: Option<PathBuf>,
}

/// Whether the database already has the `category` table.
fn has_category_table(conn: &Connection) -> rusqlite::Result<bool> {
    conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'category'", [], |r| {
        r.get::<_, i64>(0)
    })
    .map(|n| n > 0)
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("creating directory {}", parent.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("opening SQLite database {}", path.display()))?;
        Self::init(conn, Some(path.to_path_buf()))
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?, None)
    }

    /// Check `path` is usable without creating or changing anything.
    pub fn test(path: &Path) -> Result<String> {
        if !path.exists() {
            return Ok(format!(
                "{} doesn't exist yet; it will be created when you press Save & connect",
                path.display()
            ));
        }
        anyhow::ensure!(path.is_file(), "{} is not a file", path.display());
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("opening SQLite database {}", path.display()))?;
        let tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('category', 'spending')",
                [],
                |r| r.get(0),
            )
            .with_context(|| format!("{} is not an SQLite database", path.display()))?;
        if tables < 2 {
            return Ok(format!(
                "SQLite — {}: this app's tables will be added when you press Save & connect",
                path.display()
            ));
        }
        let count = |table: &str| -> Result<i64> {
            Ok(conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?)
        };
        Ok(format!(
            "SQLite — {}: {} categories, {} transactions",
            path.display(),
            count("category")?,
            count("spending")?
        ))
    }

    fn init(mut conn: Connection, path: Option<PathBuf>) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        let schema = split_schema(SCHEMA);
        // Creating the tables and seeding them is all or nothing, so a new
        // database can't be left without its default categories.
        let tx = conn.transaction()?;
        // Only a new database gets the default categories. One whose
        // categories were all deleted stays empty.
        let new = !has_category_table(&tx)?;
        for stmt in &schema.create {
            tx.execute(stmt, [])?;
        }
        if new {
            log::info!("Seeding default categories into new SQLite database");
            for stmt in &schema.seed {
                tx.execute(stmt, [])?;
            }
        }
        tx.commit()?;
        Ok(Self { conn, path })
    }

    fn in_use(&self, id: i64) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM spending WHERE bucket = ?1", [id], |r| r.get(0))?)
    }
}

impl Store for SqliteStore {
    fn describe(&self) -> String {
        match &self.path {
            Some(p) => format!("SQLite — {}", p.display()),
            None => "SQLite — in memory".into(),
        }
    }

    fn categories(&mut self) -> Result<Vec<Category>> {
        let mut stmt = self.conn.prepare("SELECT id, short FROM category ORDER BY short COLLATE NOCASE, id")?;
        let rows = stmt.query_map([], |r| Ok(Category { id: r.get(0)?, name: r.get(1)? }))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn add_category(&mut self, name: &str) -> Result<()> {
        let name = validate_category(name)?;
        ensure_unique_category(&self.categories()?, name, None)?;
        self.conn
            .execute("INSERT INTO category (short) VALUES (?1)", [name])
            .with_context(|| format!("adding category '{name}'"))?;
        Ok(())
    }

    fn rename_category(&mut self, id: i64, name: &str) -> Result<()> {
        let name = validate_category(name)?;
        ensure_unique_category(&self.categories()?, name, Some(id))?;
        self.conn
            .execute("UPDATE category SET short = ?1 WHERE id = ?2", params![name, id])
            .with_context(|| format!("renaming category to '{name}'"))?;
        Ok(())
    }

    fn delete_category(&mut self, id: i64) -> Result<()> {
        let used = self.in_use(id)?;
        anyhow::ensure!(used == 0, "Category is used by {used} transaction(s); move or delete them first");
        self.conn.execute("DELETE FROM category WHERE id = ?1", [id])?;
        Ok(())
    }

    fn spending(&mut self) -> Result<Vec<Spending>> {
        let mut stmt = self.conn.prepare(
            "SELECT line, name, date, CAST(ROUND(price * 100) AS INTEGER), bucket
             FROM spending ORDER BY date DESC, line DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get(3)?, r.get(4)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (line, name, date, pence, bucket) = row?;
            out.push(Spending { line, name, date: parse_date(&date)?, pence, bucket });
        }
        Ok(out)
    }

    fn add_spending(&mut self, s: &Spending) -> Result<()> {
        validate_spending(s)?;
        self.conn.execute(
            "INSERT INTO spending (name, date, price, bucket) VALUES (?1, ?2, ?3, ?4)",
            params![s.name.trim(), s.date.to_string(), money::to_decimal(s.pence), s.bucket],
        )?;
        Ok(())
    }

    fn update_spending(&mut self, s: &Spending) -> Result<()> {
        validate_spending(s)?;
        self.conn.execute(
            "UPDATE spending SET name = ?1, date = ?2, price = ?3, bucket = ?4 WHERE line = ?5",
            params![s.name.trim(), s.date.to_string(), money::to_decimal(s.pence), s.bucket, s.line],
        )?;
        Ok(())
    }

    fn delete_spending(&mut self, line: i64) -> Result<()> {
        self.conn.execute("DELETE FROM spending WHERE line = ?1", [line])?;
        Ok(())
    }

    fn import(&mut self, rows: &[ImportRow]) -> Result<()> {
        let existing = self.categories()?;
        let tx = self.conn.transaction()?;
        let ids = import_categories(existing, rows, |name| {
            tx.execute("INSERT INTO category (short) VALUES (?1)", [name])
                .with_context(|| format!("adding category '{name}'"))?;
            Ok(tx.last_insert_rowid())
        })?;
        {
            let mut insert = tx.prepare("INSERT INTO spending (name, date, price, bucket) VALUES (?1, ?2, ?3, ?4)")?;
            for row in rows {
                let s = row.to_spending(&ids)?;
                insert.execute(params![s.name, s.date.to_string(), money::to_decimal(s.pence), s.bucket])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}
