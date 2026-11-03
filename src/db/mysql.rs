use std::time::Duration;

use anyhow::{Context, Result};
use mysql::prelude::Queryable;
use mysql::{OptsBuilder, Pool, PoolConstraints, PoolOpts, PooledConn, TxOpts};

use super::{
    Category, ImportRow, Spending, Store, ensure_unique_category, import_categories, parse_date, split_schema,
    validate_category, validate_spending,
};
use crate::config::MySqlSettings;
use crate::money;

const SCHEMA: &str = include_str!("../../schema/mariadb.sql");

/// How long to wait for the server to accept a connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a query may wait on the server before failing, so a server that
/// stops answering can't freeze the window for good.
const IO_TIMEOUT: Duration = Duration::from_secs(20);

/// The app runs one query at a time, so it needs one connection. The pool
/// opens its minimum on creation, on the calling thread, and the default
/// minimum is 10.
const POOL_SIZE: PoolConstraints = PoolConstraints::new_const::<1, 2>();

pub struct MySqlStore {
    pool: Pool,
    label: String,
}

fn label(s: &MySqlSettings) -> String {
    format!("MariaDB/MySQL — {}@{}:{}/{}", s.user, s.host, s.port, s.database)
}

/// Connect, opening a single connection.
fn connect(s: &MySqlSettings, label: &str) -> Result<Pool> {
    log::info!("Connecting to {label}");
    let opts = OptsBuilder::new()
        .ip_or_hostname(Some(s.host.trim()))
        .tcp_port(s.port)
        .user(Some(s.user.trim()).filter(|u| !u.is_empty()))
        .pass(Some(s.password.as_str()).filter(|p| !p.is_empty()))
        .db_name(Some(s.database.trim()))
        .tcp_connect_timeout(Some(CONNECT_TIMEOUT))
        .read_timeout(Some(IO_TIMEOUT))
        .write_timeout(Some(IO_TIMEOUT))
        .pool_opts(PoolOpts::default().with_constraints(POOL_SIZE));
    Pool::new(opts).with_context(|| format!("connecting to {label}"))
}

/// How many of this app's tables exist in the connection's database.
fn our_tables(conn: &mut PooledConn) -> Result<i64> {
    let n: Option<i64> = conn.query_first(
        "SELECT COUNT(*) FROM information_schema.tables
         WHERE table_schema = DATABASE() AND table_name IN ('category', 'spending')",
    )?;
    Ok(n.unwrap_or(0))
}

impl MySqlStore {
    pub fn open(s: &MySqlSettings) -> Result<Self> {
        let label = label(s);
        let pool = connect(s, &label)?;
        let store = Self { pool, label };
        store.init()?;
        Ok(store)
    }

    /// Connect and look at the database without creating or changing anything.
    pub fn test(s: &MySqlSettings) -> Result<String> {
        let label = label(s);
        let mut conn = connect(s, &label)?.get_conn().context("getting database connection")?;
        let version: Option<String> = conn.query_first("SELECT VERSION()")?;
        let version = version.unwrap_or_default();
        Ok(if our_tables(&mut conn)? < 2 {
            format!("{label} (server {version}): this app's tables will be created when you press Save & connect")
        } else {
            let categories: Option<i64> = conn.query_first("SELECT COUNT(*) FROM category")?;
            let spending: Option<i64> = conn.query_first("SELECT COUNT(*) FROM spending")?;
            format!(
                "{label} (server {version}): {} categories, {} transactions",
                categories.unwrap_or(0),
                spending.unwrap_or(0)
            )
        })
    }

    fn conn(&self) -> Result<PooledConn> {
        self.pool.get_conn().context("getting database connection")
    }

    fn init(&self) -> Result<()> {
        let mut conn = self.conn()?;
        let schema = split_schema(SCHEMA);
        // Only a new database gets the default categories. One whose
        // categories were all deleted stays empty.
        let has_categories_table: Option<i64> = conn.query_first(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'category'",
        )?;
        let new = has_categories_table.unwrap_or(0) == 0;
        for stmt in &schema.create {
            conn.query_drop(stmt).with_context(|| format!("running schema: {stmt}"))?;
        }
        if new {
            log::info!("Seeding default categories into new MariaDB/MySQL database");
            for stmt in &schema.seed {
                conn.query_drop(stmt).with_context(|| format!("running schema: {stmt}"))?;
            }
        }
        // Tables made by 1.0.0 use the server's default character set, which
        // may not hold every character (emoji, for one). They aren't converted
        // automatically; see the README.
        let narrow: Vec<(String, String)> = conn.query(
            "SELECT table_name, table_collation FROM information_schema.tables
             WHERE table_schema = DATABASE() AND table_name IN ('category', 'spending')
               AND table_collation NOT LIKE 'utf8mb4%'",
        )?;
        for (table, collation) in narrow {
            log::warn!("Table {table} uses {collation}; some characters can't be stored. See the README to convert it.");
        }
        Ok(())
    }
}

impl Store for MySqlStore {
    fn describe(&self) -> String {
        self.label.clone()
    }

    fn categories(&mut self) -> Result<Vec<Category>> {
        Ok(self
            .conn()?
            .query_map("SELECT id, short FROM category ORDER BY short, id", |(id, name)| Category { id, name })?)
    }

    fn add_category(&mut self, name: &str) -> Result<()> {
        let name = validate_category(name)?;
        ensure_unique_category(&self.categories()?, name, None)?;
        self.conn()?
            .exec_drop("INSERT INTO category (short) VALUES (?)", (name,))
            .with_context(|| format!("adding category '{name}' (does it already exist?)"))?;
        Ok(())
    }

    fn rename_category(&mut self, id: i64, name: &str) -> Result<()> {
        let name = validate_category(name)?;
        ensure_unique_category(&self.categories()?, name, Some(id))?;
        self.conn()?
            .exec_drop("UPDATE category SET short = ? WHERE id = ?", (name, id))
            .with_context(|| format!("renaming category to '{name}' (does it already exist?)"))?;
        Ok(())
    }

    fn delete_category(&mut self, id: i64) -> Result<()> {
        let mut conn = self.conn()?;
        let used: Option<i64> = conn.exec_first("SELECT COUNT(*) FROM spending WHERE bucket = ?", (id,))?;
        let used = used.unwrap_or(0);
        anyhow::ensure!(used == 0, "Category is used by {used} transaction(s); move or delete them first");
        conn.exec_drop("DELETE FROM category WHERE id = ?", (id,))?;
        Ok(())
    }

    fn spending(&mut self) -> Result<Vec<Spending>> {
        let rows: Vec<(i64, String, String, i64, i64)> = self.conn()?.query(
            "SELECT line, name, DATE_FORMAT(date, '%Y-%m-%d'), CAST(ROUND(price * 100) AS SIGNED), bucket
             FROM spending ORDER BY date DESC, line DESC",
        )?;
        rows.into_iter()
            .map(|(line, name, date, pence, bucket)| {
                Ok(Spending { line, name, date: parse_date(&date)?, pence, bucket })
            })
            .collect()
    }

    fn add_spending(&mut self, s: &Spending) -> Result<()> {
        validate_spending(s)?;
        self.conn()?.exec_drop(
            "INSERT INTO spending (name, date, price, bucket) VALUES (?, ?, ?, ?)",
            (s.name.trim(), s.date.to_string(), money::to_decimal(s.pence), s.bucket),
        )?;
        Ok(())
    }

    fn update_spending(&mut self, s: &Spending) -> Result<()> {
        validate_spending(s)?;
        self.conn()?.exec_drop(
            "UPDATE spending SET name = ?, date = ?, price = ?, bucket = ? WHERE line = ?",
            (s.name.trim(), s.date.to_string(), money::to_decimal(s.pence), s.bucket, s.line),
        )?;
        Ok(())
    }

    fn delete_spending(&mut self, line: i64) -> Result<()> {
        self.conn()?.exec_drop("DELETE FROM spending WHERE line = ?", (line,))?;
        Ok(())
    }

    fn import(&mut self, rows: &[ImportRow]) -> Result<()> {
        let mut conn = self.conn()?;
        let mut tx = conn.start_transaction(TxOpts::default())?;
        let existing =
            tx.query_map("SELECT id, short FROM category ORDER BY short, id", |(id, name)| Category { id, name })?;
        let ids = import_categories(existing, rows, |name| {
            tx.exec_drop("INSERT INTO category (short) VALUES (?)", (name,))
                .with_context(|| format!("adding category '{name}'"))?;
            let id = tx.last_insert_id().context("the server didn't return the new category's id")?;
            Ok(i64::try_from(id)?)
        })?;
        let values = rows
            .iter()
            .map(|row| {
                let s = row.to_spending(&ids)?;
                Ok((s.name, s.date.to_string(), money::to_decimal(s.pence), s.bucket))
            })
            .collect::<Result<Vec<_>>>()?;
        tx.exec_batch("INSERT INTO spending (name, date, price, bucket) VALUES (?, ?, ?, ?)", values)?;
        // Dropping the transaction without committing rolls it back.
        tx.commit()?;
        Ok(())
    }
}
