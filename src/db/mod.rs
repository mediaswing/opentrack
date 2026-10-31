//! The budget database: built into the app (SQLite, in `sqlite.rs`, kept
//! in the app's data folder) or on a MySQL or MariaDB server (`mysql.rs`).
//!
//! Both follow the MariaDB schema the app was designed from: `category`
//! (`id`, `short`) and `spending` (`line`, `name`, `date`, `price`,
//! `bucket`), with category ids starting at 22 and the same eight starting
//! categories, named in the interface's language. A `budget` table adds an
//! optional monthly budget for each category.
//!
//! `Db` checks names, dates and amounts and writes the log, so both kinds
//! of database behave the same; each kind only stores and reads. Category
//! names are matched ignoring the case of ASCII letters, as SQLite's NOCASE
//! does, on either kind.

mod mysql;
mod sqlite;

use crate::money::{MAX_PENCE, Month, Period};
use crate::settings::Server;
use anyhow::{Result, bail};
use chrono::{Datelike, NaiveDate};
use std::cell::RefCell;
use std::path::Path;

pub(crate) use mysql::SCHEMA as MARIADB_SCHEMA;
#[cfg(test)]
pub(crate) use mysql::run_script;

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

/// The schema's `ALTER TABLE category AUTO_INCREMENT = 22`.
const FIRST_CATEGORY_ID: i64 = 22;

pub const MAX_CATEGORY_CHARS: usize = 32;
pub const MAX_NAME_CHARS: usize = 160;

/// The years a date can be in: what MySQL's DATE holds.
const YEARS: std::ops::RangeInclusive<i32> = 1000..=9999;

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

/// First and last days, both included.
type Days = (NaiveDate, NaiveDate);

fn days(period: Period) -> Days {
    (period.first_day(), period.last_day())
}

/// What each kind of database does. Everything given to it has been
/// checked by `Db`, and names trimmed.
trait Store {
    /// Every category, in alphabetical order ignoring ASCII case.
    fn categories(&mut self) -> Result<Vec<Category>>;
    fn add_category(&mut self, short: &str, budget: Option<i64>) -> Result<i64>;
    fn update_category(&mut self, id: i64, short: &str, budget: Option<i64>) -> Result<()>;
    /// How many entries each category has, leaving out those with none.
    fn entry_counts(&mut self) -> Result<Vec<(i64, i64)>>;
    /// Moves the category's entries to `move_to`, then deletes it, in one
    /// transaction. Returns how many entries moved. Fails, changing
    /// nothing, if it has entries and `move_to` is `None` or itself.
    fn delete_category(&mut self, id: i64, move_to: Option<i64>) -> Result<i64>;
    /// Entries on these days, or all of them, newest first.
    fn entries(&mut self, days: Option<Days>, bucket: Option<i64>) -> Result<Vec<Entry>>;
    /// Every entry, in order of line number.
    fn all_entries(&mut self) -> Result<Vec<Entry>>;
    fn add_entry(&mut self, entry: &Entry) -> Result<i64>;
    /// Whether the entry was there to update.
    fn update_entry(&mut self, entry: &Entry) -> Result<bool>;
    fn delete_entry(&mut self, line: i64) -> Result<()>;
    fn entry_count_between(&mut self, days: Days) -> Result<i64>;
    /// Each category's total, by id, leaving out those with no entries.
    fn totals(&mut self, days: Days) -> Result<Vec<(i64, i64)>>;
    /// Each month's total, by its key such as "2026-10", leaving out
    /// months with no entries.
    fn month_totals(&mut self, days: Days) -> Result<Vec<(String, i64)>>;
    /// Replaces everything, keeping ids, in one transaction.
    fn replace_all(&mut self, categories: &[Category], entries: &[Entry]) -> Result<()>;
    /// Adds entries whose category is given by name, found ignoring ASCII
    /// case or created, in one transaction. Returns how many were created.
    fn add_entries_by_category_name(&mut self, entries: &[(Entry, String)]) -> Result<usize>;
    /// Writes a copy of the whole database to a new or empty file.
    fn back_up(&mut self, path: &Path) -> Result<()>;
}

/// The id of the category named `short` among `categories`, ignoring
/// ASCII case.
fn find_category<'a>(categories: impl IntoIterator<Item = &'a Category>, short: &str) -> Option<i64> {
    categories.into_iter().find(|c| c.short.eq_ignore_ascii_case(short)).map(|c| c.id)
}

pub struct Db {
    /// Borrowed for each call. The window is the only user, on one thread.
    store: RefCell<Box<dyn Store>>,
    /// Where the budget is, as shown on the Import/Export tab.
    location: String,
    /// Whether the budget is on a MySQL or MariaDB server.
    on_server: bool,
}

impl Db {
    /// Opens the built-in database at `path`, making it if needed.
    pub fn open(path: &Path) -> Result<Self> {
        let store = sqlite::Sqlite::open(path)?;
        Ok(Db { store: RefCell::new(Box::new(store)), location: path.display().to_string(), on_server: false })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let store = sqlite::Sqlite::open_in_memory()?;
        Ok(Db { store: RefCell::new(Box::new(store)), location: ":memory:".into(), on_server: false })
    }

    /// Connects to a MySQL or MariaDB server, making the tables if they
    /// aren't there.
    pub fn connect(server: &Server) -> Result<Self> {
        let store = mysql::MySql::connect(server)?;
        log::info!("connected to a database server");
        Ok(Db { store: RefCell::new(Box::new(store)), location: server.describe(), on_server: true })
    }

    /// Where the budget is: the file, or the server and database.
    pub fn location(&self) -> &str {
        &self.location
    }

    /// Whether the budget is on a MySQL or MariaDB server.
    pub fn is_on_server(&self) -> bool {
        self.on_server
    }

    fn store(&self) -> std::cell::RefMut<'_, Box<dyn Store>> {
        self.store.borrow_mut()
    }

    // ----- categories ----------------------------------------------------

    /// Every category, in alphabetical order.
    pub fn categories(&self) -> Result<Vec<Category>> {
        self.store().categories()
    }

    pub fn category_by_name(&self, short: &str) -> Result<Option<i64>> {
        Ok(find_category(&self.categories()?, short.trim()))
    }

    pub fn add_category(&self, short: &str, budget: Option<i64>) -> Result<i64> {
        let short = check_category_name(short)?;
        check_budget(budget)?;
        if self.category_by_name(&short)?.is_some() {
            bail!(t!("category-exists", name = &short));
        }
        let id = self.store().add_category(&short, budget)?;
        log::info!("added category {id}");
        Ok(id)
    }

    pub fn update_category(&self, id: i64, short: &str, budget: Option<i64>) -> Result<()> {
        let short = check_category_name(short)?;
        check_budget(budget)?;
        if self.category_by_name(&short)?.is_some_and(|other| other != id) {
            bail!(t!("category-exists", name = &short));
        }
        self.store().update_category(id, &short, budget)?;
        log::info!("updated category {id}");
        Ok(())
    }

    /// How many entries are in a category.
    pub fn entry_count(&self, bucket: i64) -> Result<i64> {
        Ok(self.entry_counts()?.into_iter().find(|(id, _)| *id == bucket).map_or(0, |(_, n)| n))
    }

    /// How many entries each category has, leaving out those with none.
    pub fn entry_counts(&self) -> Result<Vec<(i64, i64)>> {
        self.store().entry_counts()
    }

    /// Whether there are no entries at all.
    pub fn has_no_entries(&self) -> Result<bool> {
        Ok(self.entry_counts()?.is_empty())
    }

    /// Deletes a category. Its entries move to `move_to` first; without one,
    /// a category that has entries can't be deleted.
    pub fn delete_category(&mut self, id: i64, move_to: Option<i64>) -> Result<()> {
        let count = self.store().delete_category(id, move_to)?;
        log::info!("deleted category {id}, moving {count} entries to {move_to:?}");
        Ok(())
    }

    // ----- spending ------------------------------------------------------

    /// The entries matching `filter`, newest first.
    pub fn entries(&self, filter: &Filter) -> Result<Vec<Entry>> {
        let entries = self.store().entries(filter.period.map(days), filter.bucket)?;
        // Matched here rather than in SQL, whose lower() differs between
        // databases and, in SQLite, only knows ASCII.
        let text = filter.text.trim().to_lowercase();
        Ok(entries.into_iter().filter(|e| text.is_empty() || e.name.to_lowercase().contains(&text)).collect())
    }

    pub fn all_entries(&self) -> Result<Vec<Entry>> {
        self.store().all_entries()
    }

    /// Adds an entry, ignoring its `line`, and returns the line it was given.
    pub fn add_entry(&self, entry: &Entry) -> Result<i64> {
        let entry = check_entry(entry)?;
        let line = self.store().add_entry(&entry)?;
        log::info!("added entry {line}");
        Ok(line)
    }

    pub fn update_entry(&self, entry: &Entry) -> Result<()> {
        let entry = check_entry(entry)?;
        if !self.store().update_entry(&entry)? {
            bail!(t!("entry-gone", line = entry.line));
        }
        log::info!("updated entry {}", entry.line);
        Ok(())
    }

    pub fn delete_entry(&self, line: i64) -> Result<()> {
        self.store().delete_entry(line)?;
        log::info!("deleted entry {line}");
        Ok(())
    }

    // ----- summaries -----------------------------------------------------

    /// How many entries are in a period.
    pub fn period_entry_count(&self, period: Period) -> Result<i64> {
        self.store().entry_count_between(days(period))
    }

    /// Each category's total for a period, keyed by category id. Categories
    /// with nothing spent are left out.
    pub fn totals(&self, period: Period) -> Result<Vec<(i64, i64)>> {
        self.store().totals(days(period))
    }

    /// The total spent in each month with a day in `period`, counting only
    /// the days in it, oldest first, including months with nothing spent.
    pub fn monthly_totals(&self, period: Period) -> Result<Vec<(Month, i64)>> {
        let rows = self.store().month_totals(days(period))?;
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
        let categories = categories
            .iter()
            .map(|c| {
                check_budget(c.budget)?;
                Ok(Category { short: check_category_name(&c.short)?, ..c.clone() })
            })
            .collect::<Result<Vec<_>>>()?;
        let entries = entries
            .iter()
            .map(|e| check_entry(e).map_err(|err| err.context(t!("context-entry", line = e.line))))
            .collect::<Result<Vec<_>>>()?;
        self.store().replace_all(&categories, &entries)?;
        log::info!("replaced the database with {} categories and {} entries", categories.len(), entries.len());
        Ok(())
    }

    /// Adds `entries`, whose `bucket` is a category name looked up or
    /// created, in one transaction. Returns how many categories were new.
    pub fn add_entries_by_category_name(&mut self, entries: &[(Entry, String)]) -> Result<usize> {
        let entries = entries
            .iter()
            .enumerate()
            .map(|(i, (entry, category))| {
                let entry = check_entry(entry).map_err(|e| e.context(t!("context-entry", line = i + 1)))?;
                Ok((entry, check_category_name(category)?))
            })
            .collect::<Result<Vec<_>>>()?;
        let created = self.store().add_entries_by_category_name(&entries)?;
        log::info!("added {} entries and {created} categories", entries.len());
        Ok(created)
    }

    /// Writes a copy of the whole built-in database to `path`, which must
    /// be new or empty. A server's database is backed up with its own
    /// tools.
    pub fn back_up(&self, path: &Path) -> Result<()> {
        self.store().back_up(path)?;
        log::info!("backed up the database");
        Ok(())
    }
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

fn check_budget(budget: Option<i64>) -> Result<()> {
    if budget.is_some_and(|pence| !(0..=MAX_PENCE).contains(&pence)) {
        bail!(t!("budget-range"));
    }
    Ok(())
}

/// The entry with its name trimmed, after checking it fits the schema.
fn check_entry(entry: &Entry) -> Result<Entry> {
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
    if !YEARS.contains(&entry.date.year()) {
        bail!(t!("entry-year-range", first = *YEARS.start(), last = *YEARS.end()));
    }
    Ok(Entry { name: name.to_owned(), ..entry.clone() })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn entry(name: &str, date: &str, price: i64, bucket: i64) -> Entry {
        Entry { line: 0, name: name.into(), date: date.parse().unwrap(), price, bucket }
    }

    /// A MySQL or MariaDB server for tests, from `OPENTRACK_TEST_MYSQL`,
    /// such as `mysql://user:password@127.0.0.1:3306`. The user needs to
    /// be able to create and drop databases. Without it, only the built-in
    /// database is tested.
    pub(crate) fn test_server() -> Option<Server> {
        let url = std::env::var("OPENTRACK_TEST_MYSQL").ok().filter(|u| !u.is_empty())?;
        let rest = url.strip_prefix("mysql://").expect("OPENTRACK_TEST_MYSQL is mysql://user:password@host:port");
        let (login, address) = rest.rsplit_once('@').expect("a user and address");
        let (user, password) = login.split_once(':').unwrap_or((login, ""));
        let (host, port) = address.trim_end_matches('/').rsplit_once(':').unwrap_or((address, "3306"));
        Some(Server {
            host: host.into(),
            port: port.parse().expect("a port number"),
            database: String::new(),
            user: user.into(),
            password: password.into(),
            // A test server on this computer seldom has a certificate.
            tls: std::env::var("OPENTRACK_TEST_MYSQL_TLS").is_ok(),
        })
    }

    /// A database made on the test server for one test, and dropped after.
    pub(crate) struct TestDatabase(pub Server);

    impl TestDatabase {
        pub(crate) fn new() -> Option<Self> {
            use std::sync::atomic::{AtomicU32, Ordering};
            static NEXT: AtomicU32 = AtomicU32::new(0);
            let mut server = test_server()?;
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
            server.database =
                format!("opentrack_test_{}_{}_{nanos}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
            mysql::admin(&server, &format!("CREATE DATABASE `{}` CHARACTER SET utf8mb4", server.database)).unwrap();
            Some(TestDatabase(server))
        }
    }

    impl Drop for TestDatabase {
        fn drop(&mut self) {
            let _ = mysql::admin(&self.0, &format!("DROP DATABASE IF EXISTS `{}`", self.0.database));
        }
    }

    /// A fresh database of each kind that can be tested, named.
    fn fresh() -> Vec<(&'static str, Db, Option<TestDatabase>)> {
        let mut all = vec![("built-in", Db::open_in_memory().unwrap(), None)];
        if let Some(test) = TestDatabase::new() {
            all.push(("server", Db::connect(&test.0).unwrap(), Some(test)));
        }
        all
    }

    #[test]
    fn starts_with_the_schema_categories() {
        for (kind, db, _keep) in fresh() {
            let categories = db.categories().unwrap();
            assert_eq!(categories.len(), 8, "{kind}");
            let mut ids: Vec<i64> = categories.iter().map(|c| c.id).collect();
            ids.sort();
            assert_eq!(ids, (22..30).collect::<Vec<_>>(), "{kind}");
            assert_eq!(db.category_by_name("food and drink").unwrap(), Some(23), "{kind}");
            assert_eq!(db.add_category("Pets", None).unwrap(), 30, "{kind}");
            let names: Vec<String> = db.categories().unwrap().into_iter().map(|c| c.short).collect();
            assert!(names.is_sorted_by_key(|n| n.to_ascii_lowercase()), "{kind} sorts by name: {names:?}");
        }
    }

    #[test]
    fn reopening_keeps_data_and_does_not_reseed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("budget.db");
        {
            let mut db = Db::open(&path).unwrap();
            db.delete_category(22, None).unwrap();
        }
        assert_eq!(Db::open(&path).unwrap().categories().unwrap().len(), 7);

        if let Some(test) = TestDatabase::new() {
            Db::connect(&test.0).unwrap().delete_category(22, None).unwrap();
            assert_eq!(Db::connect(&test.0).unwrap().categories().unwrap().len(), 7, "server");
        }
    }

    #[test]
    fn entries_filter_and_sum() {
        for (kind, db, _keep) in fresh() {
            db.add_entry(&entry("Cinema", "2026-10-02", 1200, 22)).unwrap();
            db.add_entry(&entry("Lunch", "2026-10-15", 850, 23)).unwrap();
            db.add_entry(&entry("Dinner", "2026-10-31", 2400, 23)).unwrap();
            db.add_entry(&entry("Brunch", "2026-11-01", 1000, 23)).unwrap();
            let october = Period::Month(Month { year: 2026, month: 10 });

            let all = db.entries(&Filter { period: Some(october), ..Filter::default() }).unwrap();
            assert_eq!(all.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Dinner", "Lunch", "Cinema"], "{kind}");
            let found = db.entries(&Filter { text: "NCH".into(), ..Filter::default() }).unwrap();
            assert_eq!(found.len(), 2, "{kind}");
            let food = db.entries(&Filter { period: Some(october), bucket: Some(23), ..Filter::default() }).unwrap();
            assert_eq!(food.len(), 2, "{kind}");

            assert_eq!(db.period_entry_count(october).unwrap(), 3, "{kind}");
            let mut totals = db.totals(october).unwrap();
            totals.sort();
            assert_eq!(totals, [(22, 1200), (23, 3250)], "{kind}");
            let day = |d| NaiveDate::from_ymd_opt(2026, 10, d).unwrap();
            let three_months = Period::Days { first: day(1) - chrono::Months::new(1), last: day(1) + chrono::Months::new(1) };
            let months = db.monthly_totals(three_months).unwrap();
            assert_eq!(months.iter().map(|(_, t)| *t).collect::<Vec<_>>(), [0, 4450, 1000], "{kind}");
            let part = Period::Days { first: day(10), last: day(31) };
            assert_eq!(db.period_entry_count(part).unwrap(), 2, "{kind}");
            assert_eq!(db.monthly_totals(part).unwrap(), [(Month { year: 2026, month: 10 }, 3250)], "{kind}");
        }
    }

    #[test]
    fn amounts_and_names_come_back_exactly() {
        for (kind, db, _keep) in fresh() {
            let odd = ["Café Élan ☕", "O'Neill's \"pub\"", "back\\slash"];
            for (i, name) in odd.into_iter().enumerate() {
                db.add_entry(&entry(name, "2026-10-03", [-1, MAX_PENCE, -MAX_PENCE][i], 23)).unwrap();
            }
            let back: Vec<(String, i64)> = db.all_entries().unwrap().into_iter().map(|e| (e.name, e.price)).collect();
            assert_eq!(back, [(odd[0].into(), -1), (odd[1].into(), MAX_PENCE), (odd[2].into(), -MAX_PENCE)], "{kind}");
            let found = db.entries(&Filter { text: "CAFÉ é".into(), ..Filter::default() }).unwrap();
            assert_eq!(found.len(), 1, "{kind} finds ignoring case beyond ASCII");
        }
    }

    #[test]
    fn saving_an_unchanged_entry_works() {
        for (kind, db, _keep) in fresh() {
            let line = db.add_entry(&entry("Bus", "2026-10-02", 250, 26)).unwrap();
            let same = Entry { line, ..entry("Bus", "2026-10-02", 250, 26) };
            db.update_entry(&same).unwrap_or_else(|e| panic!("{kind}: {e:#}"));
            assert!(db.update_entry(&Entry { line: line + 1, ..same }).is_err(), "{kind}: no such entry");
        }
    }

    #[test]
    fn checks_entries_against_the_schema() {
        for (kind, db, _keep) in fresh() {
            assert!(db.add_entry(&entry("  ", "2026-10-02", 1, 22)).is_err(), "{kind}");
            assert!(db.add_entry(&entry(&"x".repeat(161), "2026-10-02", 1, 22)).is_err(), "{kind}");
            assert!(db.add_entry(&entry("Too much", "2026-10-02", MAX_PENCE + 1, 22)).is_err(), "{kind}");
            assert!(db.add_entry(&entry("Long ago", "0999-12-31", 1, 22)).is_err(), "{kind}");
            assert!(db.add_entry(&entry("No such category", "2026-10-02", 1, 999)).is_err(), "{kind}");
            assert!(db.add_category("Entertainment", None).is_err(), "{kind}: names are unique, ignoring case");
            assert!(db.add_category(&"x".repeat(33), None).is_err(), "{kind}");
            assert!(db.add_category("Negative", Some(-1)).is_err(), "{kind}");
        }
    }

    #[test]
    fn deleting_a_used_category_moves_its_entries() {
        for (kind, mut db, _keep) in fresh() {
            db.add_entry(&entry("Bus", "2026-10-02", 250, 26)).unwrap();
            assert!(db.delete_category(26, None).is_err(), "{kind}");
            assert!(db.delete_category(26, Some(26)).is_err(), "{kind}");
            db.delete_category(26, Some(22)).unwrap();
            assert_eq!(db.entry_count(22).unwrap(), 1, "{kind}");
            assert_eq!(db.entry_counts().unwrap(), [(22, 1)], "{kind}");
            assert_eq!(db.category_by_name("Transport").unwrap(), None, "{kind}");
        }
    }

    #[test]
    fn budgets() {
        for (kind, db, _keep) in fresh() {
            db.update_category(23, "Food", Some(20_000)).unwrap();
            let food = db.categories().unwrap().into_iter().find(|c| c.id == 23).unwrap();
            assert_eq!((food.short.as_str(), food.budget), ("Food", Some(20_000)), "{kind}");
            db.update_category(23, "Food", None).unwrap();
            assert_eq!(db.categories().unwrap().into_iter().find(|c| c.id == 23).unwrap().budget, None, "{kind}");
            assert!(db.update_category(23, "transport", None).is_err(), "{kind}");
            let pets = db.add_category("Pets", Some(MAX_PENCE)).unwrap();
            assert_eq!(db.categories().unwrap().into_iter().find(|c| c.id == pets).unwrap().budget, Some(MAX_PENCE), "{kind}");
        }
    }

    #[test]
    fn replace_all_is_all_or_nothing() {
        for (kind, mut db, _keep) in fresh() {
            let categories = vec![Category { id: 5, short: "Only".into(), budget: Some(100) }];
            let bad = vec![entry("Fine", "2026-01-01", 1, 5), entry("Wrong category", "2026-01-01", 1, 6)];
            assert!(db.replace_all(&categories, &bad).is_err(), "{kind}");
            assert_eq!(db.categories().unwrap().len(), 8, "{kind}: nothing changed");
            db.replace_all(&categories, &bad[..1]).unwrap();
            assert_eq!(db.categories().unwrap(), categories, "{kind}");
            assert_eq!(db.all_entries().unwrap().len(), 1, "{kind}");
            assert!(db.add_category("Next", None).unwrap() > 5, "{kind}: new ids follow the copied ones");
        }
    }

    #[test]
    fn adding_by_category_name_matches_ignoring_case() {
        for (kind, mut db, _keep) in fresh() {
            let rows = vec![
                (entry("Bus", "2026-10-02", 250, 0), "TRANSPORT".to_owned()),
                (entry("Vet", "2026-10-03", 4000, 0), "Pets".to_owned()),
                (entry("Food", "2026-10-04", 900, 0), "pets".to_owned()),
            ];
            assert_eq!(db.add_entries_by_category_name(&rows).unwrap(), 1, "{kind}");
            let counts = db.entry_counts().unwrap();
            assert!(counts.contains(&(26, 1)), "{kind}: {counts:?}");
            let pets = db.category_by_name("PETS").unwrap().unwrap();
            assert_eq!(db.entry_count(pets).unwrap(), 2, "{kind}");
        }
    }

    /// What switching to a server does when it is empty: everything is
    /// copied across with its ids.
    #[test]
    fn the_built_in_database_copies_to_a_server() {
        let Some(test) = TestDatabase::new() else { return };
        let built_in = Db::open_in_memory().unwrap();
        built_in.update_category(23, "Food", Some(20_000)).unwrap();
        built_in.add_category("Pets", None).unwrap();
        built_in.add_entry(&entry("Lunch", "2026-10-15", 850, 23)).unwrap();
        built_in.delete_entry(built_in.add_entry(&entry("Gone", "2026-10-16", 1, 23)).unwrap()).unwrap();
        built_in.add_entry(&entry("Vet", "2026-10-17", -4000, 30)).unwrap();

        let mut server = Db::connect(&test.0).unwrap();
        assert!(server.has_no_entries().unwrap());
        server.replace_all(&built_in.categories().unwrap(), &built_in.all_entries().unwrap()).unwrap();
        assert_eq!(server.categories().unwrap(), built_in.categories().unwrap());
        assert_eq!(server.all_entries().unwrap(), built_in.all_entries().unwrap());
        assert!(!server.has_no_entries().unwrap());
    }

    #[test]
    fn the_password_never_reaches_debug_output() {
        let server = Server { password: "hunter2".into(), ..Server::default() };
        assert!(!format!("{server:?}").contains("hunter2"));
    }
}
