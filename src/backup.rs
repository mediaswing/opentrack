//! Backups: every category and the user's transactions in one JSON file.
//!
//! A backup restores into either backend, so it also moves data between
//! SQLite and MariaDB/MySQL. Amounts are written as decimal text (`"12.50"`),
//! never as floating point, and ids are kept, so a restore gives back exactly
//! what was saved.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::db::{Category, Spending, Store, validate_category, validate_spending};
use crate::money;

/// Written in every backup, so other JSON files are recognised as not ours.
const APP: &str = "opentrack";
/// Bumped when the layout changes in a way older versions can't read.
const FORMAT: u32 = 1;
/// Larger files are refused before they are read.
const MAX_FILE_BYTES: u64 = 200 * 1024 * 1024;
/// As enforced by `validate_category`.
const MAX_CATEGORY_CHARS: usize = 32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct File {
    app: String,
    format: u32,
    app_version: String,
    /// When the backup was made (RFC 3339, UTC).
    created: String,
    categories: Vec<FileCategory>,
    transactions: Vec<FileTransaction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FileCategory {
    id: i64,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FileTransaction {
    id: i64,
    date: String,
    description: String,
    amount: String,
    category: i64,
}

/// A backup's contents, checked and ready to restore.
#[derive(Debug, Clone, PartialEq)]
pub struct Backup {
    pub created: Option<jiff::Timestamp>,
    pub categories: Vec<Category>,
    pub spending: Vec<Spending>,
    /// Categories renamed on reading, as `(old, new)`, because their names
    /// differ from another's only by case. SQLite databases from 1.0.0 can
    /// hold such pairs, but category names are now unique ignoring case.
    pub renamed: Vec<(String, String)>,
}

impl Backup {
    /// Every category and the current user's transactions. Until other
    /// people use the database ([`Store::shared`]) that is every transaction;
    /// after, it leaves out teammates' and those without an owner, which a
    /// restore doesn't replace.
    pub fn of(store: &mut dyn Store) -> Result<Self> {
        let me = store.me().map(str::to_owned);
        let mut spending = store.spending()?;
        if store.shared()? {
            spending.retain(|s| s.owner == me);
        }
        Ok(Self { created: Some(jiff::Timestamp::now()), categories: store.categories()?, spending, renamed: Vec::new() })
    }

    /// Write the backup to `path`. The file is written beside it first and
    /// then moved into place, so a failed write never leaves half a backup.
    pub fn write(&self, path: &Path) -> Result<()> {
        let file = File {
            app: APP.into(),
            format: FORMAT,
            app_version: env!("CARGO_PKG_VERSION").into(),
            created: self.created.unwrap_or_else(jiff::Timestamp::now).to_string(),
            categories: self.categories.iter().map(|c| FileCategory { id: c.id, name: c.name.clone() }).collect(),
            transactions: self
                .spending
                .iter()
                .map(|s| FileTransaction {
                    id: s.line,
                    date: s.date.to_string(),
                    description: s.name.clone(),
                    amount: money::to_decimal(s.pence),
                    category: s.bucket,
                })
                .collect(),
        };
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let json = serde_json::to_vec_pretty(&file)?;
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, json).with_context(|| format!("writing {}", partial.display()))?;
        std::fs::rename(&partial, path).with_context(|| format!("saving {}", path.display()))?;
        log::info!(
            "Backed up {} categories and {} transactions to {}",
            self.categories.len(),
            self.spending.len(),
            path.display()
        );
        Ok(())
    }

    /// Read and check a backup. Nothing is restored from a file that has
    /// anything wrong with it.
    pub fn read(path: &Path) -> Result<Self> {
        let size = std::fs::metadata(path).with_context(|| format!("reading {}", path.display()))?.len();
        anyhow::ensure!(size <= MAX_FILE_BYTES, "{} is too large to be a backup", path.display());
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&bytes).with_context(|| format!("{} can't be restored", path.display()))
    }

    fn parse(bytes: &[u8]) -> Result<Self> {
        let value: serde_json::Value = serde_json::from_slice(bytes).context("it is not a JSON file")?;
        anyhow::ensure!(value.get("app").and_then(|a| a.as_str()) == Some(APP), "it is not an Open Budget Tracker backup");
        let format = value.get("format").and_then(|f| f.as_u64()).unwrap_or(0);
        anyhow::ensure!(
            format == u64::from(FORMAT),
            "it was made by a newer version of the app (backup format {format}); update the app to restore it"
        );
        let file: File = serde_json::from_value(value).context("the backup is damaged")?;

        let mut ids = HashSet::new();
        let mut categories = Vec::with_capacity(file.categories.len());
        for c in &file.categories {
            let name = validate_category(&c.name).with_context(|| format!("category {}", c.id))?.to_string();
            anyhow::ensure!(ids.insert(c.id), "category id {} appears twice", c.id);
            categories.push(Category { id: c.id, name });
        }
        let renamed = rename_case_duplicates(&mut categories);

        let mut lines = HashSet::new();
        let mut spending = Vec::with_capacity(file.transactions.len());
        for t in file.transactions {
            let what = || format!("transaction {}", t.id);
            let date = t.date.parse().map_err(|e| anyhow::anyhow!("invalid date '{}': {e}", t.date)).with_context(what)?;
            let pence = money::parse(&t.amount).map_err(anyhow::Error::msg).with_context(what)?;
            let s = Spending { line: t.id, name: t.description.trim().to_string(), date, pence, bucket: t.category, owner: None, team: None };
            validate_spending(&s).with_context(what)?;
            anyhow::ensure!(ids.contains(&s.bucket), "transaction {} has category {}, which isn't in the backup", t.id, t.category);
            anyhow::ensure!(lines.insert(s.line), "transaction id {} appears twice", s.line);
            spending.push(s);
        }
        Ok(Self { created: file.created.parse().ok(), categories, spending, renamed })
    }
}

/// Give each category whose name matches an earlier one's, ignoring case, a
/// name of its own: `food` after `Food` becomes `food (2)`. Its transactions
/// stay with it. Returns what was renamed.
fn rename_case_duplicates(categories: &mut [Category]) -> Vec<(String, String)> {
    let mut taken: HashSet<String> = categories.iter().map(|c| c.name.to_lowercase()).collect();
    let mut seen = HashSet::new();
    let mut renamed = Vec::new();
    for c in categories.iter_mut() {
        if seen.insert(c.name.to_lowercase()) {
            continue;
        }
        let new = (2..)
            .map(|n| {
                let suffix = format!(" ({n})");
                let room = MAX_CATEGORY_CHARS - suffix.chars().count();
                let base: String = c.name.chars().take(room).collect();
                format!("{}{suffix}", base.trim_end())
            })
            .find(|name| !taken.contains(&name.to_lowercase()))
            .expect("some number is free");
        log::warn!("Backup has categories '{}' that differ only by case; restoring one as '{new}'", c.name);
        taken.insert(new.to_lowercase());
        seen.insert(new.to_lowercase());
        renamed.push((std::mem::replace(&mut c.name, new.clone()), new));
    }
    renamed
}

/// Where a copy of the data goes before a restore replaces it.
pub fn safety_copy_path() -> PathBuf {
    let stamp = jiff::Zoned::now().strftime("%Y-%m-%d-%H%M%S").to_string();
    config::data_dir().join("backups").join(format!("before-restore-{stamp}.json"))
}

/// A file name for a new backup, e.g. `opentrack-backup-2026-11-03.json`.
pub fn suggested_name() -> String {
    format!("opentrack-backup-{}.json", jiff::Zoned::now().date())
}

#[cfg(test)]
mod tests {
    use jiff::civil::Date;

    use super::*;
    use crate::db::SqliteStore;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("opentrack-backup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn round_trip_restores_exactly() {
        let mut db = SqliteStore::open_in_memory().unwrap();
        db.add_category("Café ☕").unwrap();
        let cat = db.categories().unwrap().into_iter().find(|c| c.name == "Café ☕").unwrap().id;
        for (name, pence) in [("Flat white", 340), ("Refund", -340), ("Beans", 99_999_999)] {
            let s = Spending { line: 0, name: name.into(), date: Date::new(2026, 11, 3).unwrap(), pence, bucket: cat, owner: None, team: None };
            db.add_spending(&s).unwrap();
        }
        let before = Backup::of(&mut db).unwrap();
        let dir = temp("roundtrip");
        let path = dir.join("sub").join("b.json");
        before.write(&path).unwrap();
        assert!(!path.with_extension("json.partial").exists());

        // Restore into a database with different data in it.
        let mut other = SqliteStore::open_in_memory().unwrap();
        other.add_category("Gone").unwrap();
        let read = Backup::read(&path).unwrap();
        assert_eq!(read.categories, before.categories);
        assert_eq!(read.spending, before.spending);
        other.replace_all(&read.categories, &read.spending).unwrap();
        assert_eq!(other.categories().unwrap(), before.categories);
        assert_eq!(other.spending().unwrap(), before.spending);

        // New rows don't reuse restored ids.
        other.add_category("New").unwrap();
        let ids: HashSet<i64> = other.categories().unwrap().iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), before.categories.len() + 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// SQLite databases from 1.0.0 can hold `Food` and `food`; their backups
    /// must still restore, with nothing lost.
    #[test]
    fn restores_categories_that_differ_only_by_case() {
        let long = "A".repeat(32);
        let json = serde_json::json!({
            "app": "opentrack", "format": 1, "app_version": "1.0.0", "created": "2026-11-03T00:00:00Z",
            "categories": [
                {"id": 22, "name": "Food"}, {"id": 23, "name": "food"}, {"id": 24, "name": "FOOD"},
                {"id": 25, "name": "Food (2)"}, {"id": 26, "name": long}, {"id": 27, "name": long.to_lowercase()}
            ],
            "transactions": [{"id": 1, "date": "2026-11-03", "description": "Lunch", "amount": "1.00", "category": 23}]
        });
        let b = Backup::parse(&serde_json::to_vec(&json).unwrap()).unwrap();
        let names: Vec<&str> = b.categories.iter().map(|c| c.name.as_str()).collect();
        let short_long = format!("{} (2)", "a".repeat(28));
        assert_eq!(names, vec!["Food", "food (3)", "FOOD (4)", "Food (2)", long.as_str(), short_long.as_str()]);
        assert_eq!(b.renamed.len(), 3);
        assert!(names.iter().all(|n| validate_category(n).is_ok()));

        let mut db = SqliteStore::open_in_memory().unwrap();
        db.replace_all(&b.categories, &b.spending).unwrap();
        assert_eq!(db.categories().unwrap().len(), 6);
        assert_eq!(db.spending().unwrap()[0].bucket, 23, "the transaction keeps its category");
    }

    #[test]
    fn refuses_bad_backups() {
        let good = |f: &mut serde_json::Value| {
            *f = serde_json::json!({
                "app": "opentrack", "format": 1, "app_version": "1.0.0", "created": "2026-11-03T00:00:00Z",
                "categories": [{"id": 22, "name": "Food"}],
                "transactions": [{"id": 1, "date": "2026-11-03", "description": "Lunch", "amount": "12.50", "category": 22}]
            });
        };
        let check = |change: &dyn Fn(&mut serde_json::Value)| {
            let mut f = serde_json::Value::Null;
            good(&mut f);
            change(&mut f);
            Backup::parse(&serde_json::to_vec(&f).unwrap())
        };
        let push = |list: &mut serde_json::Value, item| list.as_array_mut().unwrap().push(item);
        let ok = check(&|_| {}).unwrap();
        assert_eq!(ok.spending[0].pence, 1250);

        assert!(Backup::parse(b"not json").is_err());
        assert!(check(&|f| f["app"] = "other".into()).unwrap_err().to_string().contains("not an Open Budget"));
        assert!(check(&|f| f["format"] = 2.into()).unwrap_err().to_string().contains("newer version"));
        assert!(check(&|f| f["transactions"][0]["category"] = 99.into()).is_err());
        assert!(check(&|f| f["transactions"][0]["date"] = "2026-02-30".into()).is_err());
        assert!(check(&|f| f["transactions"][0]["amount"] = "lots".into()).is_err());
        assert!(check(&|f| f["transactions"][0]["description"] = "".into()).is_err());
        assert!(check(&|f| push(&mut f["categories"], serde_json::json!({"id": 22, "name": "Other"}))).is_err());
        assert!(check(&|f| {
            let copy = f["transactions"][0].clone();
            push(&mut f["transactions"], copy)
        }).is_err());
        assert!(check(&|f| f["categories"].as_array_mut().unwrap().clear()).is_err());
    }
}
