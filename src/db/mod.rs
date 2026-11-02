//! Storage layer. Both backends implement [`Store`] against the same schema.

mod mysql;
mod sqlite;

use anyhow::Result;
use jiff::civil::Date;

use crate::config::{Backend, Config};

pub use self::mysql::MySqlStore;
pub use self::sqlite::SqliteStore;

#[derive(Debug, Clone, PartialEq)]
pub struct Category {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spending {
    /// Row id (`line` column). Ignored when inserting.
    pub line: i64,
    pub name: String,
    pub date: Date,
    /// Price in pence.
    pub pence: i64,
    /// Category id (`bucket` column).
    pub bucket: i64,
}

pub trait Store {
    /// Human-readable description of where the data lives.
    fn describe(&self) -> String;

    fn categories(&mut self) -> Result<Vec<Category>>;
    fn add_category(&mut self, name: &str) -> Result<()>;
    fn rename_category(&mut self, id: i64, name: &str) -> Result<()>;
    fn delete_category(&mut self, id: i64) -> Result<()>;

    /// All spending, newest first.
    fn spending(&mut self) -> Result<Vec<Spending>>;
    fn add_spending(&mut self, s: &Spending) -> Result<()>;
    fn update_spending(&mut self, s: &Spending) -> Result<()>;
    fn delete_spending(&mut self, line: i64) -> Result<()>;
}

/// Open the store selected in the config, creating tables and seed data as needed.
pub fn open(cfg: &Config) -> Result<Box<dyn Store>> {
    let store: Box<dyn Store> = match cfg.backend {
        Backend::Sqlite => Box::new(SqliteStore::open(&cfg.sqlite_path)?),
        Backend::MySql => Box::new(MySqlStore::open(&cfg.mysql)?),
    };
    log::info!("Opened database: {}", store.describe());
    Ok(store)
}

/// Check the database selected in `cfg` can be used, without changing it:
/// no file, table or seed data is created. Returns a description for the user.
pub fn test(cfg: &Config) -> Result<String> {
    match cfg.backend {
        Backend::Sqlite => SqliteStore::test(&cfg.sqlite_path),
        Backend::MySql => MySqlStore::test(&cfg.mysql),
    }
}

/// A schema file split into statements that always run (`CREATE ...`) and
/// statements that only run on a fresh database (seed data).
pub(crate) struct Schema {
    pub create: Vec<String>,
    pub seed: Vec<String>,
}

pub(crate) fn split_schema(sql: &str) -> Schema {
    let without_comments: String =
        sql.lines().filter(|l| !l.trim_start().starts_with("--")).collect::<Vec<_>>().join("\n");
    let mut schema = Schema { create: Vec::new(), seed: Vec::new() };
    for stmt in without_comments.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        if stmt.to_ascii_uppercase().starts_with("CREATE") {
            schema.create.push(stmt.to_string());
        } else {
            schema.seed.push(stmt.to_string());
        }
    }
    schema
}

pub(crate) fn validate_category(name: &str) -> Result<&str> {
    let name = name.trim();
    anyhow::ensure!(!name.is_empty(), "Category name cannot be empty");
    anyhow::ensure!(name.chars().count() <= 32, "Category name must be at most 32 characters");
    Ok(name)
}

/// Category names are unique ignoring case, on both backends. (MariaDB's
/// collation already works that way; SQLite databases from 1.0.0 don't.)
/// `except` is the category being renamed, which may change its own case.
pub(crate) fn ensure_unique_category(existing: &[Category], name: &str, except: Option<i64>) -> Result<()> {
    let lower = name.to_lowercase();
    if let Some(clash) = existing.iter().find(|c| Some(c.id) != except && c.name.to_lowercase() == lower) {
        anyhow::bail!("A category called '{}' already exists", clash.name);
    }
    Ok(())
}

pub(crate) fn validate_spending(s: &Spending) -> Result<()> {
    anyhow::ensure!(!s.name.trim().is_empty(), "Description cannot be empty");
    anyhow::ensure!(s.name.chars().count() <= 160, "Description must be at most 160 characters");
    anyhow::ensure!(s.pence.abs() <= crate::money::MAX_PENCE, "Amount is too large");
    Ok(())
}

pub(crate) fn parse_date(text: &str) -> Result<Date> {
    // SQLite may hold a full timestamp if edited externally; take the date part.
    let day = text.get(..10).unwrap_or(text);
    day.parse::<Date>().map_err(|e| anyhow::anyhow!("invalid date '{text}' in database: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_mariadb_schema() {
        let s = split_schema(include_str!("../../schema/mariadb.sql"));
        assert_eq!(s.create.len(), 2);
        assert_eq!(s.seed.len(), 2); // ALTER + INSERT
    }

    #[test]
    fn sqlite_store_roundtrip() {
        let mut db = SqliteStore::open_in_memory().unwrap();
        let cats = db.categories().unwrap();
        assert_eq!(cats.len(), 8);
        assert_eq!(cats.iter().map(|c| c.id).min(), Some(22));

        let food = cats.iter().find(|c| c.name == "Food and Drink").unwrap().id;
        let mut s = Spending {
            line: 0,
            name: "Lunch".into(),
            date: Date::new(2026, 11, 2).unwrap(),
            pence: 1099,
            bucket: food,
        };
        db.add_spending(&s).unwrap();
        let rows = db.spending().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pence, 1099);
        assert_eq!(rows[0].date, s.date);

        s.line = rows[0].line;
        s.pence = 1;
        db.update_spending(&s).unwrap();
        assert_eq!(db.spending().unwrap()[0].pence, 1);

        assert!(db.delete_category(food).is_err(), "in-use category must not delete");
        db.delete_spending(s.line).unwrap();
        db.delete_category(food).unwrap();
        db.add_category("Pets").unwrap();
        assert!(db.add_category("Pets").is_err());
        assert!(db.add_category("pets").is_err(), "names are unique ignoring case");
        let pets = db.categories().unwrap().into_iter().find(|c| c.name == "Pets").unwrap().id;
        db.rename_category(pets, "PETS").unwrap();
        assert!(db.rename_category(pets, "transport").is_err());
    }

    /// A temporary folder for a test's files, removed afterwards.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("opentrack-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn default_categories_only_seed_a_new_database() {
        let dir = TempDir::new("seed");
        let path = dir.0.join("seed.db");
        {
            let mut db = SqliteStore::open(&path).unwrap();
            for c in db.categories().unwrap() {
                db.delete_category(c.id).unwrap();
            }
        }
        let mut db = SqliteStore::open(&path).unwrap();
        assert!(db.categories().unwrap().is_empty(), "deleted defaults must not come back");
    }

    #[test]
    fn testing_sqlite_changes_nothing() {
        let dir = TempDir::new("probe");
        let path = dir.0.join("sub").join("new.db");
        let msg = SqliteStore::test(&path).unwrap();
        assert!(msg.contains("will be created"), "{msg}");
        assert!(!dir.0.exists(), "testing must not create folders or files");

        SqliteStore::open(&path).unwrap();
        let msg = SqliteStore::test(&path).unwrap();
        assert!(msg.contains("8 categories"), "{msg}");

        std::fs::write(dir.0.join("junk.db"), b"this is not a database at all, not even close......").unwrap();
        assert!(SqliteStore::test(&dir.0.join("junk.db")).is_err());
    }
}
