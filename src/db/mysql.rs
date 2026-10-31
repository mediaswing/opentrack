//! A MySQL or MariaDB server, in the schema the app was designed from:
//! prices as `DECIMAL(8,2)` and dates as `DATE`. Tables already there, such
//! as ones loaded from the SQL export, are used as they are; missing ones
//! are made.
//!
//! Amounts go to the server as text such as "12.50" and come back as whole
//! pence, worked out by the server, so nothing passes through floating
//! point. Names match ignoring the case of ASCII letters, as in the
//! built-in database, while the `short` column itself compares exactly
//! (see `transfer::export_sql`).

use super::{Category, Days, Entry, FIRST_CATEGORY_ID, STARTING_CATEGORIES, Store, find_category};
use crate::money;
use crate::settings::Server;
use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use mysql::consts::CapabilityFlags;
use mysql::prelude::Queryable;
use mysql::{Conn, OptsBuilder, SslOpts, TxOpts};
use std::path::Path;
use std::time::Duration;

/// The tables, as made on a server and written by the SQL export. The
/// categories' AUTO_INCREMENT is `FIRST_CATEGORY_ID`.
pub const SCHEMA: [&str; 3] = [
    "CREATE TABLE IF NOT EXISTS `category` (
    `id` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
    `short` VARCHAR(32) COLLATE utf8mb4_bin NOT NULL UNIQUE
) AUTO_INCREMENT = 22 ENGINE = InnoDB DEFAULT CHARSET = utf8mb4",
    "CREATE TABLE IF NOT EXISTS `spending` (
    `line` INT NOT NULL PRIMARY KEY AUTO_INCREMENT,
    `name` VARCHAR(160) NOT NULL,
    `date` DATE NOT NULL,
    `price` DECIMAL(8,2) NOT NULL,
    `bucket` INT NOT NULL,
    KEY `spending_date` (`date`),
    FOREIGN KEY (`bucket`) REFERENCES `category` (`id`)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4",
    "CREATE TABLE IF NOT EXISTS `budget` (
    `bucket` INT NOT NULL PRIMARY KEY,
    `monthly` DECIMAL(8,2) NOT NULL,
    FOREIGN KEY (`bucket`) REFERENCES `category` (`id`) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4",
];

const _: () = assert!(FIRST_CATEGORY_ID == 22, "SCHEMA's AUTO_INCREMENT");

/// How long to wait for the server to answer a connection, and a query.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Every session works the same, whatever the server's own settings:
/// text is utf8mb4, and a value that doesn't fit is an error rather than
/// being cut short. Tables must be InnoDB, for transactions.
const SESSION: [&str; 2] = [
    "SET NAMES utf8mb4",
    "SET SESSION sql_mode = 'STRICT_TRANS_TABLES,NO_ZERO_IN_DATE,NO_ZERO_DATE,ERROR_FOR_DIVISION_BY_ZERO,NO_ENGINE_SUBSTITUTION'",
];

const ENTRY_COLUMNS: &str = "line, name, DATE_FORMAT(date, '%Y-%m-%d'), CAST(price * 100 AS SIGNED), bucket";

/// The first and last dates a `DATE` holds, for listing every entry.
const ALL_DAYS: (&str, &str) = ("1000-01-01", "9999-12-31");

pub struct MySql {
    conn: Conn,
}

fn options(server: &Server, database: Option<&str>) -> OptsBuilder {
    let opts = OptsBuilder::new()
        .ip_or_hostname(Some(server.host.trim()))
        .tcp_port(server.port)
        .user(Some(server.user.trim()))
        .pass(Some(server.password.as_str()))
        .db_name(database)
        // The address given, not a local socket the server names.
        .prefer_socket(false)
        .tcp_connect_timeout(Some(CONNECT_TIMEOUT))
        .read_timeout(Some(QUERY_TIMEOUT))
        .write_timeout(Some(QUERY_TIMEOUT))
        // An UPDATE counts the rows it found, not only those it changed, so
        // saving an entry unchanged doesn't look like it has gone.
        .additional_capabilities(CapabilityFlags::CLIENT_FOUND_ROWS)
        .init(SESSION.to_vec());
    if server.tls { opts.ssl_opts(SslOpts::default()) } else { opts }
}

/// Runs one statement without choosing a database, such as making one for
/// a test.
#[cfg(test)]
pub fn admin(server: &Server, sql: &str) -> Result<()> {
    Conn::new(options(server, None))?.query_drop(sql)?;
    Ok(())
}

impl MySql {
    pub fn connect(server: &Server) -> Result<Self> {
        if server.host.trim().is_empty() {
            bail!(t!("server-host-missing"));
        }
        let database = server.database.trim();
        if database.is_empty() {
            bail!(t!("server-database-missing"));
        }
        let conn = Conn::new(options(server, Some(database))).map_err(|e| {
            // Newer MariaDB servers turn TLS on with a certificate they made
            // themselves, which nothing trusts, so say what to do about it.
            let message = format!("{}: {e}", t!("error-connect-server", server = server.describe()));
            if server.tls && e.to_string().contains("certificate") {
                anyhow::anyhow!("{message}\n\n{}", t!("server-tls-hint"))
            } else {
                anyhow::anyhow!(message)
            }
        })?;
        let mut db = MySql { conn };
        db.setup().with_context(|| t!("error-server-tables", server = server.describe()))?;
        Ok(db)
    }

    /// Makes the tables that aren't there, and the starting categories if
    /// the categories table is new.
    fn setup(&mut self) -> Result<()> {
        let existed: i64 = self
            .conn
            .query_first(
                "SELECT COUNT(*) FROM information_schema.tables
                 WHERE table_schema = DATABASE() AND table_name = 'category'",
            )?
            .unwrap_or(0);
        for statement in SCHEMA {
            self.conn.query_drop(statement)?;
        }
        let not_innodb: Vec<String> = self.conn.query(
            "SELECT table_name FROM information_schema.tables
             WHERE table_schema = DATABASE() AND table_name IN ('category', 'spending', 'budget')
               AND engine <> 'InnoDB'",
        )?;
        if !not_innodb.is_empty() {
            bail!(t!("server-not-innodb", tables = not_innodb.join(", ")));
        }
        // MySQL commits each CREATE TABLE as it goes, so the categories
        // can't share a transaction with them as in SQLite.
        if existed == 0 {
            log::info!("creating the database schema on the server");
            let mut tx = self.conn.start_transaction(TxOpts::default())?;
            for id in STARTING_CATEGORIES {
                tx.exec_drop("INSERT INTO category (short) VALUES (?)", (crate::i18n::message(id, None),))?;
            }
            tx.commit()?;
        }
        Ok(())
    }
}

fn categories(q: &mut impl Queryable) -> Result<Vec<Category>> {
    let mut categories: Vec<Category> = q.query_map(
        "SELECT c.id, c.short, CAST(b.monthly * 100 AS SIGNED) FROM category c LEFT JOIN budget b ON b.bucket = c.id",
        |(id, short, budget)| Category { id, short, budget },
    )?;
    // As SQLite's NOCASE orders them.
    categories.sort_by_cached_key(|c| c.short.to_ascii_lowercase());
    Ok(categories)
}

fn set_budget(q: &mut impl Queryable, id: i64, budget: Option<i64>) -> Result<()> {
    q.exec_drop("DELETE FROM budget WHERE bucket = ?", (id,))?;
    if let Some(pence) = budget {
        q.exec_drop("INSERT INTO budget (bucket, monthly) VALUES (?, ?)", (id, money::plain(pence)))?;
    }
    Ok(())
}

type EntryRow = (i64, String, String, i64, i64);

fn entries_from_rows(rows: Vec<EntryRow>) -> Result<Vec<Entry>> {
    rows.into_iter()
        .map(|(line, name, date, price, bucket)| {
            let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d").with_context(|| format!("entry {line} has date {date:?}"))?;
            Ok(Entry { line, name, date, price, bucket })
        })
        .collect()
}

fn day_params((first, last): Days) -> (String, String) {
    (first.to_string(), last.to_string())
}

/// An entry's values for INSERT and UPDATE: name, date, price and bucket.
fn entry_params(e: &Entry) -> (String, String, String, i64) {
    (e.name.clone(), e.date.to_string(), money::plain(e.price), e.bucket)
}

impl Store for MySql {
    fn categories(&mut self) -> Result<Vec<Category>> {
        categories(&mut self.conn)
    }

    fn add_category(&mut self, short: &str, budget: Option<i64>) -> Result<i64> {
        let mut tx = self.conn.start_transaction(TxOpts::default())?;
        tx.exec_drop("INSERT INTO category (short) VALUES (?)", (short,))?;
        let id = tx.last_insert_id().context("no id for the new category")? as i64;
        set_budget(&mut tx, id, budget)?;
        tx.commit()?;
        Ok(id)
    }

    fn update_category(&mut self, id: i64, short: &str, budget: Option<i64>) -> Result<()> {
        let mut tx = self.conn.start_transaction(TxOpts::default())?;
        tx.exec_drop("UPDATE category SET short = ? WHERE id = ?", (short, id))?;
        set_budget(&mut tx, id, budget)?;
        tx.commit()?;
        Ok(())
    }

    fn entry_counts(&mut self) -> Result<Vec<(i64, i64)>> {
        Ok(self.conn.query("SELECT bucket, COUNT(*) FROM spending GROUP BY bucket")?)
    }

    fn delete_category(&mut self, id: i64, move_to: Option<i64>) -> Result<i64> {
        let mut tx = self.conn.start_transaction(TxOpts::default())?;
        let count: i64 = tx.exec_first("SELECT COUNT(*) FROM spending WHERE bucket = ?", (id,))?.unwrap_or(0);
        if count > 0 {
            match move_to {
                Some(to) if to != id => tx.exec_drop("UPDATE spending SET bucket = ? WHERE bucket = ?", (to, id))?,
                // Dropping the transaction rolls it back.
                _ => bail!(t!("db-move-entries-first", count = count)),
            }
        }
        tx.exec_drop("DELETE FROM category WHERE id = ?", (id,))?;
        tx.commit()?;
        Ok(count)
    }

    fn entries(&mut self, days: Option<Days>, bucket: Option<i64>) -> Result<Vec<Entry>> {
        let (first, last) = days.map_or((ALL_DAYS.0.to_owned(), ALL_DAYS.1.to_owned()), day_params);
        let rows = self.conn.exec(
            format!(
                "SELECT {ENTRY_COLUMNS} FROM spending
                 WHERE date BETWEEN ? AND ? AND (? IS NULL OR bucket = ?)
                 ORDER BY date DESC, line DESC"
            ),
            (first, last, bucket, bucket),
        )?;
        entries_from_rows(rows)
    }

    fn all_entries(&mut self) -> Result<Vec<Entry>> {
        let rows = self.conn.query(format!("SELECT {ENTRY_COLUMNS} FROM spending ORDER BY line"))?;
        entries_from_rows(rows)
    }

    fn add_entry(&mut self, e: &Entry) -> Result<i64> {
        self.conn.exec_drop("INSERT INTO spending (name, date, price, bucket) VALUES (?, ?, ?, ?)", entry_params(e))?;
        Ok(self.conn.last_insert_id() as i64)
    }

    fn update_entry(&mut self, e: &Entry) -> Result<bool> {
        let (name, date, price, bucket) = entry_params(e);
        self.conn.exec_drop(
            "UPDATE spending SET name = ?, date = ?, price = ?, bucket = ? WHERE line = ?",
            (name, date, price, bucket, e.line),
        )?;
        Ok(self.conn.affected_rows() > 0)
    }

    fn delete_entry(&mut self, line: i64) -> Result<()> {
        self.conn.exec_drop("DELETE FROM spending WHERE line = ?", (line,))?;
        Ok(())
    }

    fn entry_count_between(&mut self, days: Days) -> Result<i64> {
        Ok(self.conn.exec_first("SELECT COUNT(*) FROM spending WHERE date BETWEEN ? AND ?", day_params(days))?.unwrap_or(0))
    }

    fn totals(&mut self, days: Days) -> Result<Vec<(i64, i64)>> {
        Ok(self.conn.exec(
            "SELECT bucket, CAST(SUM(price) * 100 AS SIGNED) FROM spending WHERE date BETWEEN ? AND ? GROUP BY bucket",
            day_params(days),
        )?)
    }

    fn month_totals(&mut self, days: Days) -> Result<Vec<(String, i64)>> {
        Ok(self.conn.exec(
            "SELECT DATE_FORMAT(date, '%Y-%m') AS month, CAST(SUM(price) * 100 AS SIGNED) FROM spending
             WHERE date BETWEEN ? AND ? GROUP BY month",
            day_params(days),
        )?)
    }

    fn replace_all(&mut self, categories: &[Category], entries: &[Entry]) -> Result<()> {
        let mut tx = self.conn.start_transaction(TxOpts::default())?;
        // DELETE rather than TRUNCATE, which would commit at once.
        for table in ["spending", "budget", "category"] {
            tx.query_drop(format!("DELETE FROM {table}"))?;
        }
        for c in categories {
            tx.exec_drop("INSERT INTO category (id, short) VALUES (?, ?)", (c.id, &c.short))
                .with_context(|| t!("context-category", id = c.id, name = &c.short))?;
            if let Some(pence) = c.budget {
                tx.exec_drop("INSERT INTO budget (bucket, monthly) VALUES (?, ?)", (c.id, money::plain(pence)))
                    .with_context(|| t!("context-budget", name = &c.short))?;
            }
        }
        for e in entries {
            let (name, date, price, bucket) = entry_params(e);
            tx.exec_drop(
                "INSERT INTO spending (line, name, date, price, bucket) VALUES (?, ?, ?, ?, ?)",
                (e.line, name, date, price, bucket),
            )
            .with_context(|| t!("context-entry", line = e.line))?;
        }
        tx.commit()?;
        Ok(())
    }

    fn add_entries_by_category_name(&mut self, entries: &[(Entry, String)]) -> Result<usize> {
        let mut tx = self.conn.start_transaction(TxOpts::default())?;
        let mut known = categories(&mut tx)?;
        let mut created = 0;
        for (i, (e, short)) in entries.iter().enumerate() {
            let bucket = match find_category(&known, short) {
                Some(id) => id,
                None => {
                    tx.exec_drop("INSERT INTO category (short) VALUES (?)", (short,))?;
                    created += 1;
                    let id = tx.last_insert_id().context("no id for the new category")? as i64;
                    known.push(Category { id, short: short.clone(), budget: None });
                    id
                }
            };
            let (name, date, price, _) = entry_params(e);
            tx.exec_drop("INSERT INTO spending (name, date, price, bucket) VALUES (?, ?, ?, ?)", (name, date, price, bucket))
                .with_context(|| t!("context-entry", line = i + 1))?;
        }
        tx.commit()?;
        Ok(created)
    }

    fn back_up(&mut self, _path: &Path) -> Result<()> {
        bail!(t!("server-no-copy"))
    }
}
