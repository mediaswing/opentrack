use std::time::Duration;

use anyhow::{Context, Result};
use mysql::prelude::Queryable;
use mysql::{OptsBuilder, Pool, PoolConstraints, PoolOpts, PooledConn, TxOpts};

use super::{
    Category, ImportRow, Spending, Store, Team, ensure_unique_category, import_categories, parse_date, split_schema,
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
    /// The current user's identity ([`crate::identity`]). Their own
    /// transactions carry it in the `owner` column.
    me: String,
}

/// The rows the current user can see: their own, those shared with a team
/// they are in, and those from before owners were recorded. Takes the
/// user's identity twice.
const VISIBLE: &str = "(owner = ? OR owner IS NULL OR team IN (SELECT team FROM team_member WHERE member = ?))";

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

/// Whether `table` in the connection's database has `column`.
fn has_column(conn: &mut PooledConn, table: &str, column: &str) -> Result<bool> {
    let n: Option<i64> = conn.exec_first(
        "SELECT COUNT(*) FROM information_schema.columns
         WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?",
        (table, column),
    )?;
    Ok(n.unwrap_or(0) > 0)
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
        let me = crate::identity::resolve(&s.identity)?;
        log::info!("Signed-in user: {me}");
        Self::open_as(s, me)
    }

    /// Open as the user with identity `me`.
    fn open_as(s: &MySqlSettings, me: String) -> Result<Self> {
        let label = label(s);
        let pool = connect(s, &label)?;
        let store = Self { pool, label, me };
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
        // Spending tables made before owners were recorded get the columns
        // now. Their rows have no owner, so everyone can still see them.
        if !has_column(&mut conn, "spending", "owner")? {
            log::info!("Adding owner and team columns to the spending table");
            let added = conn.query_drop(
                "ALTER TABLE spending
                 ADD COLUMN owner VARCHAR(190) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NULL,
                 ADD COLUMN team INT NULL, ADD KEY (owner),
                 ADD FOREIGN KEY (team) REFERENCES team (id) ON DELETE SET NULL",
            );
            // Someone else connecting at the same moment may have added them first.
            if let Err(e) = added {
                anyhow::ensure!(
                    has_column(&mut conn, "spending", "owner")?,
                    "adding owner and team columns to the spending table: {e}"
                );
                log::info!("The owner and team columns were added by another connection");
            }
        }
        // Identities must match exactly, as the app compares them, so that
        // `Sam` and `sam` aren't one person. Columns added by the first
        // version with owners compared them ignoring case (and, on MySQL 8,
        // accents).
        for (table, column, key) in [("spending", "owner", "NULL"), ("team_member", "member", "NOT NULL")] {
            let collation: Option<Option<String>> = conn.exec_first(
                "SELECT collation_name FROM information_schema.columns
                 WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?",
                (table, column),
            )?;
            if collation.flatten().as_deref() != Some("utf8mb4_bin") {
                log::info!("Making {table}.{column} compare exactly");
                conn.query_drop(format!(
                    "ALTER TABLE {table} MODIFY {column} VARCHAR(190) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin {key}"
                ))
                .with_context(|| format!("making {table}.{column} compare exactly"))?;
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

impl MySqlStore {
    /// Check the current user may change transaction `line`: it must exist,
    /// and be theirs or have no owner.
    fn ensure_can_change(&self, conn: &mut impl Queryable, line: i64) -> Result<()> {
        let row: Option<(Option<String>,)> = conn.exec_first(
            format!("SELECT owner FROM spending WHERE line = ? AND {VISIBLE}"),
            (line, &self.me, &self.me),
        )?;
        match row {
            None => anyhow::bail!("Transaction #{line} doesn't exist; it may have been deleted"),
            Some((Some(owner),)) if owner != self.me => {
                anyhow::bail!("Transaction #{line} was shared by someone else; only they can change it")
            }
            Some(_) => Ok(()),
        }
    }

    fn shared_in(&self, conn: &mut impl Queryable) -> Result<bool> {
        let shared: Option<bool> = conn.exec_first(
            "SELECT EXISTS (SELECT 1 FROM spending WHERE owner IS NOT NULL AND owner <> ?)
                 OR EXISTS (SELECT 1 FROM team)",
            (&self.me,),
        )?;
        Ok(shared.unwrap_or(true))
    }

    /// Restore as 1.1.0 did, for a database nobody else uses yet: replace
    /// every category and transaction, keeping their ids. The transactions
    /// become the user's.
    fn replace_everything(
        &self,
        tx: &mut mysql::Transaction<'_>,
        categories: Vec<(i64, String)>,
        spending: &[Spending],
    ) -> Result<()> {
        let spending: Vec<_> = spending
            .iter()
            .map(|s| (s.line, s.name.trim().to_string(), s.date.to_string(), money::to_decimal(s.pence), s.bucket, &self.me))
            .collect();
        // DELETE rather than TRUNCATE, which would commit straight away.
        tx.query_drop("DELETE FROM spending")?;
        tx.query_drop("DELETE FROM category")?;
        tx.exec_batch("INSERT INTO category (id, short) VALUES (?, ?)", categories).context("restoring categories")?;
        tx.exec_batch("INSERT INTO spending (line, name, date, price, bucket, owner) VALUES (?, ?, ?, ?, ?, ?)", spending)
            .context("restoring transactions")?;
        Ok(())
    }

    /// Check `team` is unset or one the current user belongs to.
    fn ensure_my_team(&self, conn: &mut impl Queryable, team: Option<i64>) -> Result<()> {
        let Some(team) = team else { return Ok(()) };
        let n: Option<i64> =
            conn.exec_first("SELECT COUNT(*) FROM team_member WHERE team = ? AND member = ?", (team, &self.me))?;
        anyhow::ensure!(n.unwrap_or(0) > 0, "You can only share with a team you are in");
        Ok(())
    }
}

impl Store for MySqlStore {
    fn describe(&self) -> String {
        self.label.clone()
    }

    fn me(&self) -> Option<&str> {
        Some(&self.me)
    }

    fn teams(&mut self) -> Result<Vec<Team>> {
        Ok(self.conn()?.exec_map(
            "SELECT t.id, t.name FROM team t JOIN team_member m ON m.team = t.id WHERE m.member = ? ORDER BY t.name, t.id",
            (&self.me,),
            |(id, name)| Team { id, name },
        )?)
    }

    fn shared(&mut self) -> Result<bool> {
        let mut conn = self.conn()?;
        self.shared_in(&mut conn)
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
        // Categories are shared, so other people's transactions count too.
        let used: Option<(i64, i64)> = conn.exec_first(
            format!("SELECT COUNT(*), CAST(COALESCE(SUM({VISIBLE}), 0) AS SIGNED) FROM spending WHERE bucket = ?"),
            (&self.me, &self.me, id),
        )?;
        let (used, seen) = used.unwrap_or((0, 0));
        if used > 0 {
            let hidden = used - seen;
            let others = if hidden > 0 { format!(", {hidden} of them other people's") } else { String::new() };
            anyhow::bail!("Category is used by {used} transaction(s){others}; they must be moved or deleted first");
        }
        conn.exec_drop("DELETE FROM category WHERE id = ?", (id,))?;
        Ok(())
    }

    fn spending(&mut self) -> Result<Vec<Spending>> {
        type Row = (i64, String, String, i64, i64, Option<String>, Option<i64>);
        let rows: Vec<Row> = self.conn()?.exec(
            format!(
                "SELECT line, name, DATE_FORMAT(date, '%Y-%m-%d'), CAST(ROUND(price * 100) AS SIGNED), bucket, owner, team
                 FROM spending WHERE {VISIBLE} ORDER BY date DESC, line DESC"
            ),
            (&self.me, &self.me),
        )?;
        rows.into_iter()
            .map(|(line, name, date, pence, bucket, owner, team)| {
                Ok(Spending { line, name, date: parse_date(&date)?, pence, bucket, owner, team })
            })
            .collect()
    }

    fn add_spending(&mut self, s: &Spending) -> Result<()> {
        validate_spending(s)?;
        let mut conn = self.conn()?;
        self.ensure_my_team(&mut conn, s.team)?;
        conn.exec_drop(
            "INSERT INTO spending (name, date, price, bucket, owner, team) VALUES (?, ?, ?, ?, ?, ?)",
            (s.name.trim(), s.date.to_string(), money::to_decimal(s.pence), s.bucket, &self.me, s.team),
        )?;
        Ok(())
    }

    fn update_spending(&mut self, s: &Spending) -> Result<()> {
        validate_spending(s)?;
        let mut conn = self.conn()?;
        let mut tx = conn.start_transaction(TxOpts::default())?;
        self.ensure_can_change(&mut tx, s.line)?;
        self.ensure_my_team(&mut tx, s.team)?;
        tx.exec_drop(
            "UPDATE spending SET name = ?, date = ?, price = ?, bucket = ?, team = ?
             WHERE line = ? AND (owner = ? OR owner IS NULL)",
            (s.name.trim(), s.date.to_string(), money::to_decimal(s.pence), s.bucket, s.team, s.line, &self.me),
        )?;
        tx.commit()?;
        Ok(())
    }

    fn delete_spending(&mut self, line: i64) -> Result<()> {
        let mut conn = self.conn()?;
        let mut tx = conn.start_transaction(TxOpts::default())?;
        self.ensure_can_change(&mut tx, line)?;
        tx.exec_drop("DELETE FROM spending WHERE line = ? AND (owner = ? OR owner IS NULL)", (line, &self.me))?;
        tx.commit()?;
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
                Ok((s.name, s.date.to_string(), money::to_decimal(s.pence), s.bucket, &self.me))
            })
            .collect::<Result<Vec<_>>>()?;
        tx.exec_batch("INSERT INTO spending (name, date, price, bucket, owner) VALUES (?, ?, ?, ?, ?)", values)?;
        // Dropping the transaction without committing rolls it back.
        tx.commit()?;
        Ok(())
    }

    fn replace_all(&mut self, categories: &[Category], spending: &[Spending]) -> Result<()> {
        let categories = categories
            .iter()
            .map(|c| Ok((c.id, validate_category(&c.name)?.to_string())))
            .collect::<Result<Vec<_>>>()?;
        for s in spending {
            validate_spending(s)?;
        }
        let mut conn = self.conn()?;
        let mut tx = conn.start_transaction(TxOpts::default())?;
        if !self.shared_in(&mut tx)? {
            self.replace_everything(&mut tx, categories, spending)?;
            tx.commit()?;
            return Ok(());
        }
        // Other people use this database, so only the user's own
        // transactions are replaced and no category is removed.
        let existing =
            tx.query_map("SELECT id, short FROM category ORDER BY short, id", |(id, name)| Category { id, name })?;
        let mut by_name: std::collections::HashMap<String, i64> =
            existing.into_iter().map(|c| (c.name.to_lowercase(), c.id)).collect();
        // The backup's category ids, mapped to this database's.
        let mut ids = std::collections::HashMap::new();
        for (backup_id, name) in &categories {
            let id = match by_name.get(&name.to_lowercase()) {
                Some(id) => *id,
                None => {
                    tx.exec_drop("INSERT INTO category (short) VALUES (?)", (name,))
                        .with_context(|| format!("restoring category '{name}'"))?;
                    let id = i64::try_from(tx.last_insert_id().context("the server didn't return the new category's id")?)?;
                    by_name.insert(name.to_lowercase(), id);
                    id
                }
            };
            ids.insert(*backup_id, id);
        }
        let values = spending
            .iter()
            .map(|s| {
                let bucket = *ids.get(&s.bucket).with_context(|| format!("transaction {} has no category", s.line))?;
                Ok((s.name.trim().to_string(), s.date.to_string(), money::to_decimal(s.pence), bucket, &self.me))
            })
            .collect::<Result<Vec<_>>>()?;
        tx.exec_drop("DELETE FROM spending WHERE owner = ?", (&self.me,))?;
        tx.exec_batch("INSERT INTO spending (name, date, price, bucket, owner) VALUES (?, ?, ?, ?, ?)", values)
            .context("restoring transactions")?;
        tx.commit()?;
        Ok(())
    }
}

/// Run against a real server when `OPENTRACK_TEST_MYSQL` names a database,
/// as `user:password@host:port/database`, that the tests may empty. Without
/// it they pass without checking anything.
#[cfg(test)]
mod tests {
    use jiff::civil::Date;

    use super::*;

    fn settings() -> Option<MySqlSettings> {
        let url = std::env::var("OPENTRACK_TEST_MYSQL").ok()?;
        let (auth, rest) = url.rsplit_once('@')?;
        let (user, password) = auth.split_once(':').unwrap_or((auth, ""));
        let (addr, database) = rest.split_once('/')?;
        let (host, port) = addr.split_once(':').unwrap_or((addr, "3306"));
        Some(MySqlSettings {
            host: host.into(),
            port: port.parse().ok()?,
            database: database.into(),
            user: user.into(),
            password: password.into(),
            identity: String::new(),
        })
    }

    /// The tests share one database, so they take turns.
    static DATABASE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Empty the test database and run `setup` in it, while holding it.
    fn fresh(cfg: &MySqlSettings, setup: &[&str]) -> std::sync::MutexGuard<'static, ()> {
        let guard = DATABASE.lock().unwrap_or_else(|e| e.into_inner());
        let mut conn = connect(cfg, "test").unwrap().get_conn().unwrap();
        for t in ["spending", "team_member", "team", "category"] {
            conn.query_drop(format!("DROP TABLE IF EXISTS {t}")).unwrap();
        }
        for stmt in setup {
            conn.query_drop(stmt).unwrap();
        }
        guard
    }

    /// The tables as version 1.1.0 made them, before owners were recorded.
    const V1_1: [&str; 4] = [
        "CREATE TABLE category (id INT NOT NULL PRIMARY KEY AUTO_INCREMENT, short VARCHAR(32) NOT NULL UNIQUE)",
        "CREATE TABLE spending (line INT NOT NULL PRIMARY KEY AUTO_INCREMENT, name VARCHAR(160) NOT NULL, date DATE NOT NULL, price DECIMAL(8,2) NOT NULL, bucket INT NOT NULL, FOREIGN KEY (bucket) REFERENCES category (id))",
        "INSERT INTO category (short) VALUES ('Old')",
        "INSERT INTO spending (name, date, price, bucket) VALUES ('Legacy', '2026-01-01', 1.00, 1)",
    ];

    fn collation(store: &MySqlStore, table: &str, column: &str) -> String {
        let c: Option<String> = store
            .conn()
            .unwrap()
            .exec_first(
                "SELECT collation_name FROM information_schema.columns
                 WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?",
                (table, column),
            )
            .unwrap();
        c.unwrap()
    }

    #[test]
    fn identities_compare_exactly() {
        let Some(cfg) = settings() else { return };
        // Tables as the first version with owners made them: identities
        // compared ignoring case.
        let _db = fresh(&cfg, &[
            "CREATE TABLE category (id INT NOT NULL PRIMARY KEY AUTO_INCREMENT, short VARCHAR(32) NOT NULL UNIQUE) DEFAULT CHARSET = utf8mb4",
            "CREATE TABLE team (id INT NOT NULL PRIMARY KEY AUTO_INCREMENT, name VARCHAR(64) NOT NULL UNIQUE) DEFAULT CHARSET = utf8mb4",
            "CREATE TABLE team_member (team INT NOT NULL, member VARCHAR(190) NOT NULL, PRIMARY KEY (team, member), KEY (member), FOREIGN KEY (team) REFERENCES team (id) ON DELETE CASCADE) DEFAULT CHARSET = utf8mb4",
            "CREATE TABLE spending (line INT NOT NULL PRIMARY KEY AUTO_INCREMENT, name VARCHAR(160) NOT NULL, date DATE NOT NULL, price DECIMAL(8,2) NOT NULL, bucket INT NOT NULL, owner VARCHAR(190) NULL, team INT NULL, KEY (owner), FOREIGN KEY (bucket) REFERENCES category (id), FOREIGN KEY (team) REFERENCES team (id) ON DELETE SET NULL) DEFAULT CHARSET = utf8mb4",
            "INSERT INTO category (short) VALUES ('Food')",
            "INSERT INTO team VALUES (1, 'Household')",
            "INSERT INTO team_member VALUES (1, 'Sam')",
            "INSERT INTO spending (name, date, price, bucket, owner, team) VALUES ('Sam private', '2026-11-01', 1.00, 1, 'Sam', NULL), ('Sam shared', '2026-11-01', 1.00, 1, 'Sam', 1)",
        ]);
        let mut sam = MySqlStore::open_as(&cfg, "sam".into()).unwrap();
        assert_eq!(collation(&sam, "spending", "owner"), "utf8mb4_bin");
        assert_eq!(collation(&sam, "team_member", "member"), "utf8mb4_bin");

        // `sam` is not `Sam`: none of Sam's rows or teams are theirs.
        assert!(names(&mut sam).is_empty());
        assert!(sam.teams().unwrap().is_empty());
        sam.add_spending(&tx("sam's", 1, None)).unwrap();
        // Restoring as `sam` leaves Sam's transactions alone.
        sam.replace_all(&[Category { id: 1, name: "Food".into() }], &[]).unwrap();
        let mut upper = MySqlStore::open_as(&cfg, "Sam".into()).unwrap();
        assert_eq!(names(&mut upper), ["Sam private", "Sam shared"]);
        assert!(names(&mut sam).is_empty());
    }

    #[test]
    fn alone_on_an_upgraded_database_works_as_before() {
        let Some(cfg) = settings() else { return };
        let _db = fresh(&cfg, &V1_1);
        let mut alice = MySqlStore::open_as(&cfg, "alice@one".into()).unwrap();
        alice.add_spending(&tx("New", 1, None)).unwrap();
        assert!(!alice.shared().unwrap(), "nobody else has used it");

        // The backup has the transactions from before owners, too.
        let backup = crate::backup::Backup::of(&mut alice).unwrap();
        let mut backed_up: Vec<&str> = backup.spending.iter().map(|s| s.name.as_str()).collect();
        backed_up.sort();
        assert_eq!(backed_up, ["Legacy", "New"]);

        // A restore gives back exactly what was saved, without doubling the
        // old transactions, and leaves nothing that wasn't in the backup.
        alice.add_category("Extra").unwrap();
        alice.add_spending(&tx("After", 1, None)).unwrap();
        alice.replace_all(&backup.categories, &backup.spending).unwrap();
        assert_eq!(alice.categories().unwrap(), backup.categories);
        let lines = |store: &mut MySqlStore| {
            let mut l: Vec<(i64, String)> = store.spending().unwrap().into_iter().map(|s| (s.line, s.name)).collect();
            l.sort();
            l
        };
        let mut expected: Vec<(i64, String)> = backup.spending.iter().map(|s| (s.line, s.name.clone())).collect();
        expected.sort();
        assert_eq!(lines(&mut alice), expected);

        // Once someone else adds a transaction, backups and restores are
        // only Alice's own.
        let mut bob = MySqlStore::open_as(&cfg, "bob@two".into()).unwrap();
        bob.add_spending(&tx("Bob's", 1, None)).unwrap();
        assert!(alice.shared().unwrap());
        let mine = crate::backup::Backup::of(&mut alice).unwrap();
        assert_eq!(mine.spending.len(), 2, "Legacy and New, restored as Alice's");
        alice.replace_all(&mine.categories, &[]).unwrap();
        assert_eq!(names(&mut bob), ["Bob's"]);
        assert!(names(&mut alice).is_empty());
    }

    #[test]
    fn a_team_makes_the_database_shared() {
        let Some(cfg) = settings() else { return };
        let _db = fresh(&cfg, &V1_1);
        let mut alice = MySqlStore::open_as(&cfg, "alice@one".into()).unwrap();
        assert!(!alice.shared().unwrap());
        alice.conn().unwrap().query_drop("INSERT INTO team (name) VALUES ('Household')").unwrap();
        assert!(alice.shared().unwrap());
    }

    #[test]
    fn simultaneous_upgrades_all_connect() {
        let Some(cfg) = settings() else { return };
        for _ in 0..3 {
            let _db = fresh(&cfg, &V1_1);
            let opened: Vec<_> = (0..4)
                .map(|i| {
                    let cfg = cfg.clone();
                    std::thread::spawn(move || MySqlStore::open_as(&cfg, format!("user{i}")).map(|_| ()))
                })
                .collect();
            for t in opened {
                t.join().unwrap().unwrap();
            }
        }
    }

    fn tx(name: &str, bucket: i64, team: Option<i64>) -> Spending {
        Spending { line: 0, name: name.into(), date: Date::new(2026, 11, 3).unwrap(), pence: 500, bucket, owner: None, team }
    }

    fn names(store: &mut MySqlStore) -> Vec<String> {
        let mut n: Vec<String> = store.spending().unwrap().into_iter().map(|s| s.name).collect();
        n.sort();
        n
    }

    #[test]
    fn users_see_only_their_own_and_their_teams() {
        let Some(cfg) = settings() else { return };
        // A database from before owners were recorded, with one row.
        let _db = fresh(&cfg, &V1_1);
        let mut alice = MySqlStore::open_as(&cfg, "S-1-5-21-1-1001".into()).unwrap();
        let mut bob = MySqlStore::open_as(&cfg, "bob".into()).unwrap();
        let mut carol = MySqlStore::open_as(&cfg, "carol".into()).unwrap();
        {
            let mut conn = alice.conn().unwrap();
            conn.query_drop("INSERT INTO team (id, name) VALUES (1, 'Household')").unwrap();
            conn.query_drop("INSERT INTO team_member VALUES (1, 'S-1-5-21-1-1001'), (1, 'bob')").unwrap();
        }
        assert_eq!(alice.teams().unwrap(), vec![Team { id: 1, name: "Household".into() }]);
        assert!(carol.teams().unwrap().is_empty());

        let cat = alice.categories().unwrap()[0].id;
        alice.add_spending(&tx("Alice private", cat, None)).unwrap();
        alice.add_spending(&tx("Alice shared", cat, Some(1))).unwrap();
        bob.add_spending(&tx("Bob private", cat, None)).unwrap();
        assert!(carol.add_spending(&tx("Carol sneaky", cat, Some(1))).is_err(), "not her team");
        carol.add_spending(&tx("Carol private", cat, None)).unwrap();

        assert_eq!(names(&mut alice), ["Alice private", "Alice shared", "Legacy"]);
        assert_eq!(names(&mut bob), ["Alice shared", "Bob private", "Legacy"]);
        assert_eq!(names(&mut carol), ["Carol private", "Legacy"]);

        // Bob sees Alice's shared one but can't change or delete it.
        let shared = bob.spending().unwrap().into_iter().find(|s| s.name == "Alice shared").unwrap();
        assert_eq!(shared.owner.as_deref(), Some("S-1-5-21-1-1001"));
        assert!(bob.update_spending(&Spending { pence: 1, ..shared.clone() }).is_err());
        assert!(bob.delete_spending(shared.line).is_err());
        // Nor can Carol touch Alice's private one, which she can't even see.
        let private = alice.spending().unwrap().into_iter().find(|s| s.name == "Alice private").unwrap();
        assert!(carol.delete_spending(private.line).is_err());
        // Alice can, including making it private again.
        alice.update_spending(&Spending { team: None, ..shared.clone() }).unwrap();
        assert_eq!(names(&mut bob), ["Bob private", "Legacy"]);

        // An in-use category says other people's transactions count.
        let err = carol.delete_category(cat).unwrap_err().to_string();
        assert!(err.contains("other people's"), "{err}");

        // Imports belong to the importer.
        bob.import(&[ImportRow { name: "Bob import".into(), date: Date::new(2026, 11, 3).unwrap(), pence: 1, category: "New".into() }]).unwrap();
        assert!(!names(&mut alice).contains(&"Bob import".to_string()));

        // Restoring replaces only Carol's own transactions.
        let backup = vec![Spending { line: 77, bucket: 900, ..tx("Carol restored", 0, None) }];
        carol.replace_all(&[Category { id: 900, name: "old".into() }], &backup).unwrap();
        assert_eq!(names(&mut carol), ["Carol restored", "Legacy"]);
        assert_eq!(names(&mut alice), ["Alice private", "Alice shared", "Legacy"]);
        let restored = carol.spending().unwrap().into_iter().find(|s| s.name == "Carol restored").unwrap();
        assert_eq!(restored.bucket, cat, "matched the existing category by name");
        assert_eq!(restored.owner.as_deref(), Some("carol"));
    }
}
