//! A speech-first text interface.
//!
//! Every message is written so it reads well through a screen reader or a
//! text-to-speech engine:
//! - one complete sentence (or short phrase) per line, ending in a full stop;
//! - no tables, columns, box drawing, colours or emoji;
//! - lists say how many items they hold before reading them;
//! - dates are written out in words ("Wednesday 18 November 2026");
//! - negative amounts say "minus" instead of using a hyphen;
//! - every prompt says what to type, and pressing Enter on an empty line
//!   cancels the current task.

use std::io::{self, BufRead, Write};

use chrono::{Datelike, Duration, Local, Months, NaiveDate};
use log::{debug, error, info};

use crate::db::{Category, Db, Spending};
use crate::locale::{DateOrder, LocaleFormat};

pub struct Ui<R: BufRead, W: Write> {
    db: Db,
    fmt: LocaleFormat,
    input: R,
    out: W,
}

enum Flow {
    Continue,
    Quit,
}

impl<R: BufRead, W: Write> Ui<R, W> {
    pub fn new(db: Db, fmt: LocaleFormat, input: R, out: W) -> Self {
        Self { db, fmt, input, out }
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.say(&format!("Welcome to {}.", crate::APP_TITLE))?;
        self.menu()?;
        while let Some(choice) = self.ask("Main menu. Type a number or command, or help to hear the options:")? {
            debug!("menu choice {choice:?}");
            let result = match choice.to_lowercase().as_str() {
                "" => Ok(Flow::Continue),
                "1" | "add" | "a" => self.add_spending().map(|_| Flow::Continue),
                "2" | "list" | "l" => self.list_month().map(|_| Flow::Continue),
                "3" | "summary" | "s" => self.summary().map(|_| Flow::Continue),
                "4" | "delete" | "d" => self.delete_spending().map(|_| Flow::Continue),
                "5" | "categories" | "c" => self.list_categories().map(|_| Flow::Continue),
                "6" | "new" | "new category" | "n" => self.add_category().map(|_| Flow::Continue),
                "7" | "help" | "h" | "?" | "menu" => self.menu().map(|_| Flow::Continue),
                "0" | "quit" | "q" | "exit" => Ok(Flow::Quit),
                other => {
                    self.say(&format!("Sorry, \"{other}\" is not a menu option. Type help to hear the options."))?;
                    Ok(Flow::Continue)
                }
            };
            if let Flow::Quit = result? {
                break;
            }
        }
        info!("session ended");
        self.say("Goodbye.")
    }

    // ----- input and output helpers -------------------------------------

    fn say(&mut self, line: &str) -> io::Result<()> {
        writeln!(self.out, "{line}")?;
        self.out.flush()
    }

    /// Print a prompt and read one line. Returns `None` at end of input.
    fn ask(&mut self, prompt: &str) -> io::Result<Option<String>> {
        write!(self.out, "{prompt} ")?;
        self.out.flush()?;
        let mut line = String::new();
        if self.input.read_line(&mut line)? == 0 {
            writeln!(self.out)?;
            return Ok(None);
        }
        Ok(Some(line.trim().to_string()))
    }

    /// Like `ask`, but an empty answer (or end of input) means "cancel".
    fn ask_or_cancel(&mut self, prompt: &str) -> io::Result<Option<String>> {
        match self.ask(prompt)? {
            Some(s) if !s.is_empty() => Ok(Some(s)),
            _ => {
                debug!("prompt cancelled");
                self.say("Cancelled. Nothing was changed.")?;
                Ok(None)
            }
        }
    }

    fn db_error(&mut self, e: rusqlite::Error) -> io::Result<()> {
        error!("database error: {e}");
        self.say(&format!("Sorry, the database reported a problem: {e}."))
    }

    fn menu(&mut self) -> io::Result<()> {
        for line in [
            "There are 8 options.",
            "1, add spending.",
            "2, list a month's spending.",
            "3, summary of a month by category.",
            "4, delete a spending entry.",
            "5, categories, to hear the list of categories.",
            "6, new category.",
            "7, help, which repeats these options.",
            "0, quit.",
            "You can also type the first word of an option, such as add or summary.",
            "While answering a question, press Enter on an empty line to cancel.",
        ] {
            self.say(line)?;
        }
        Ok(())
    }

    // ----- tasks ----------------------------------------------------------

    fn add_spending(&mut self) -> io::Result<()> {
        self.say("Add spending.")?;
        let name = loop {
            let Some(name) = self.ask_or_cancel("What did you buy?")? else { return Ok(()) };
            if name.chars().count() <= 160 {
                break name;
            }
            self.say("That name is too long. Please use 160 characters or fewer.")?;
        };
        let pence = loop {
            let prompt = format!("How much was it? For example, {}:", self.fmt.example_amount());
            let Some(text) = self.ask_or_cancel(&prompt)? else { return Ok(()) };
            match self.fmt.parse_money(&text) {
                Ok(p) => break p,
                Err(msg) => {
                    debug!("amount rejected");
                    self.say(&msg)?
                }
            }
        };
        let date = loop {
            let prompt = format!(
                "What date? Press Enter for today, or type yesterday, or a date such as {}:",
                self.fmt.short_date(today())
            );
            let Some(text) = self.ask(&prompt)? else {
                return self.say("Cancelled. Nothing was changed.");
            };
            match parse_date(&text, today(), self.fmt.date_order) {
                Ok(d) => break d,
                Err(()) => {
                    debug!("date rejected");
                    let msg = format!(
                        "\"{text}\" is not a date I understand. Try a date such as {}.",
                        self.fmt.short_date(today())
                    );
                    self.say(&msg)?
                }
            }
        };
        let Some(category) = self.choose_category()? else { return Ok(()) };

        match self.db.add_spending(&name, date, pence, category.id) {
            Ok(line) => {
                info!("added entry {line} in category {}", category.id);
                let entry = Spending { line, name, date, pence, category: category.short };
                let text = format!("Saved. {}", self.describe(&entry));
                self.say(&text)
            }
            Err(e) => self.db_error(e),
        }
    }

    fn choose_category(&mut self) -> io::Result<Option<Category>> {
        let cats = match self.db.categories() {
            Ok(c) => c,
            Err(e) => {
                self.db_error(e)?;
                return Ok(None);
            }
        };
        self.say(&format!("Categories are: {}.", spoken_categories(&cats)))?;
        loop {
            let Some(text) = self.ask_or_cancel("Which category? Type its number or name:")? else { return Ok(None) };
            match match_category(&cats, &text) {
                Ok(c) => return Ok(Some(c.clone())),
                Err(msg) => self.say(&msg)?,
            }
        }
    }

    fn list_month(&mut self) -> io::Result<()> {
        let Some((from, to)) = self.ask_month()? else { return Ok(()) };
        let entries = match self.db.spending_between(from, to) {
            Ok(e) => e,
            Err(e) => return self.db_error(e),
        };
        debug!("listed {} entries from {from} to {to}", entries.len());
        let month = self.fmt.month_year(from);
        if entries.is_empty() {
            return self.say(&format!("There is no spending recorded for {month}."));
        }
        let total: i64 = entries.iter().map(|e| e.pence).sum();
        self.say(&format!(
            "{} for {month}, totalling {}.",
            plural(entries.len() as i64, "entry", "entries"),
            self.fmt.money(total)
        ))?;
        for e in &entries {
            let text = self.describe(e);
            self.say(&text)?;
        }
        self.say("End of list.")
    }

    fn summary(&mut self) -> io::Result<()> {
        let Some((from, to)) = self.ask_month()? else { return Ok(()) };
        let totals = match self.db.totals_between(from, to) {
            Ok(t) => t,
            Err(e) => return self.db_error(e),
        };
        debug!("summarised {} categories from {from} to {to}", totals.len());
        let month = self.fmt.month_year(from);
        if totals.is_empty() {
            return self.say(&format!("There is no spending recorded for {month}."));
        }
        let total: i64 = totals.iter().map(|t| t.pence).sum();
        let count: i64 = totals.iter().map(|t| t.count).sum();
        self.say(&format!(
            "Summary for {month}. You spent {} across {}, in {}.",
            self.fmt.money(total),
            plural(count, "entry", "entries"),
            plural(totals.len() as i64, "category", "categories"),
        ))?;
        for t in &totals {
            let share = if total > 0 && t.pence >= 0 {
                format!(", which is {} percent of the total", (t.pence * 100 + total / 2) / total)
            } else {
                String::new()
            };
            self.say(&format!(
                "{}: {}, from {}{share}.",
                t.category,
                self.fmt.money(t.pence),
                plural(t.count, "entry", "entries"),
            ))?;
        }
        self.say("End of summary.")
    }

    fn delete_spending(&mut self) -> io::Result<()> {
        self.say("Delete a spending entry. You can find entry numbers by listing a month.")?;
        let Some(text) = self.ask_or_cancel("Entry number to delete:")? else { return Ok(()) };
        let Ok(line) = text.trim_start_matches('#').parse::<i64>() else {
            return self.say(&format!("\"{text}\" is not an entry number. Nothing was changed."));
        };
        let entry = match self.db.get_spending(line) {
            Ok(Some(e)) => e,
            Ok(None) => return self.say(&format!("There is no entry number {line}. Nothing was changed.")),
            Err(e) => return self.db_error(e),
        };
        let text = self.describe(&entry);
        self.say(&text)?;
        let answer = self.ask("Delete this entry? Type yes to confirm:")?.unwrap_or_default();
        if !matches!(answer.to_lowercase().as_str(), "yes" | "y") {
            return self.say("Not deleted. Nothing was changed.");
        }
        match self.db.delete_spending(line) {
            Ok(_) => {
                info!("deleted entry {line}");
                self.say(&format!("Entry {line} deleted."))
            }
            Err(e) => self.db_error(e),
        }
    }

    fn list_categories(&mut self) -> io::Result<()> {
        match self.db.categories() {
            Ok(cats) => {
                self.say(&format!("There are {}.", plural(cats.len() as i64, "category", "categories")))?;
                for c in &cats {
                    self.say(&format!("{}, {}.", c.id, c.short))?;
                }
                self.say("End of list.")
            }
            Err(e) => self.db_error(e),
        }
    }

    fn add_category(&mut self) -> io::Result<()> {
        self.say("New category.")?;
        let Some(name) = self.ask_or_cancel("Name of the new category, up to 32 characters:")? else { return Ok(()) };
        if name.chars().count() > 32 {
            return self.say("That name is too long. Nothing was changed.");
        }
        match self.db.add_category(&name) {
            Ok(id) => {
                info!("added category {id}");
                self.say(&format!("Added category {id}, {name}."))
            }
            Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
                self.say(&format!("A category called {name} already exists. Nothing was changed."))
            }
            Err(e) => self.db_error(e),
        }
    }

    /// Ask which month to report on. Returns the first day of that month and
    /// the first day of the following month.
    fn ask_month(&mut self) -> io::Result<Option<(NaiveDate, NaiveDate)>> {
        loop {
            let example = self.fmt.example_month(today());
            let prompt =
                format!("Which month? Press Enter for this month, or type last, or a month such as {example}:");
            let Some(text) = self.ask(&prompt)? else {
                self.say("Cancelled.")?;
                return Ok(None);
            };
            match parse_month(&text, today()) {
                Ok(start) => return Ok(Some((start, start + Months::new(1)))),
                Err(()) => {
                    debug!("month rejected");
                    let msg = format!(
                        "\"{}\" is not a month I understand. Try a month such as {example}, or a month name such as November.",
                        text.trim()
                    );
                    self.say(&msg)?
                }
            }
        }
    }

    fn describe(&self, e: &Spending) -> String {
        format!(
            "Entry {}: {}, {}, on {}, in {}.",
            e.line,
            e.name,
            self.fmt.money(e.pence),
            self.fmt.long_date(e.date),
            e.category
        )
    }
}

// ----- pure helpers (tested below) -----------------------------------------

fn today() -> NaiveDate {
    Local::now().date_naive()
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn spoken_categories(cats: &[Category]) -> String {
    cats.iter().map(|c| format!("{} {}", c.id, c.short)).collect::<Vec<_>>().join(", ")
}

/// Match by id, by exact name, or by a name prefix that only one category has.
fn match_category<'a>(cats: &'a [Category], text: &str) -> Result<&'a Category, String> {
    let text = text.trim();
    if let Ok(id) = text.parse::<i64>() {
        return cats.iter().find(|c| c.id == id).ok_or_else(|| format!("There is no category number {id}."));
    }
    let lower = text.to_lowercase();
    if let Some(c) = cats.iter().find(|c| c.short.to_lowercase() == lower) {
        return Ok(c);
    }
    let matches: Vec<&Category> = cats.iter().filter(|c| c.short.to_lowercase().starts_with(&lower)).collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => Err(format!("No category matches \"{text}\".")),
        many => Err(format!(
            "\"{text}\" matches {} categories: {}. Please be more specific.",
            many.len(),
            many.iter().map(|c| c.short.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

/// Parse a typed date. Numbers may be separated by any non-digit
/// character. A four-digit first number means year first; otherwise the
/// locale's order is used. A missing year means the current year.
fn parse_date(text: &str, today: NaiveDate, order: DateOrder) -> Result<NaiveDate, ()> {
    let t = text.trim().to_lowercase();
    match t.as_str() {
        "" | "today" => return Ok(today),
        "yesterday" => return Ok(today - Duration::days(1)),
        _ => {}
    }
    // Words such as "soon" are not dates; non-Latin date markers (年, 月) are fine.
    if t.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err(());
    }
    let parts: Vec<&str> = t.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).collect();
    // Anything longer than four digits can't be part of a date.
    if parts.iter().any(|p| p.len() > 4) {
        return Err(());
    }
    let nums: Vec<i32> = parts.iter().map(|p| p.parse().map_err(|_| ())).collect::<Result<_, _>>()?;
    let year = |y: i32, digits: usize| if digits <= 2 { 2000 + y } else { y };
    let (y, m, d) = match (nums.as_slice(), order) {
        ([y, m, d], _) if parts[0].len() == 4 => (*y, *m, *d),
        ([a, b, c], DateOrder::YearMonthDay) => (year(*a, parts[0].len()), *b, *c),
        ([a, b, c], DateOrder::MonthDayYear) => (year(*c, parts[2].len()), *a, *b),
        ([a, b, c], DateOrder::DayMonthYear) => (year(*c, parts[2].len()), *b, *a),
        ([a, b], DateOrder::DayMonthYear) => (today.year(), *b, *a),
        ([a, b], _) => (today.year(), *a, *b),
        _ => return Err(()),
    };
    let (m, d) = (u32::try_from(m).map_err(|_| ())?, u32::try_from(d).map_err(|_| ())?);
    NaiveDate::from_ymd_opt(y, m, d).ok_or(())
}

/// Returns the first day of the chosen month. Empty means this month.
/// Accepts 11/2026, 2026-11, an English month name, or a name and year.
fn parse_month(text: &str, today: NaiveDate) -> Result<NaiveDate, ()> {
    let this = today.with_day(1).unwrap();
    let t = text.trim().to_lowercase();
    match t.as_str() {
        "" | "this" | "this month" => return Ok(this),
        "last" | "last month" => return Ok(this - Months::new(1)),
        _ => {}
    }
    let parts: Vec<&str> = t.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).collect();
    if parts.len() == 2 && !t.chars().any(|c| c.is_ascii_alphabetic()) {
        let (y, m) = if parts[0].len() == 4 { (parts[0], parts[1]) } else { (parts[1], parts[0]) };
        return NaiveDate::from_ymd_opt(y.parse().map_err(|_| ())?, m.parse().map_err(|_| ())?, 1).ok_or(());
    }
    let with_year = if t.split_whitespace().count() == 1 { format!("1 {t} {}", today.year()) } else { format!("1 {t}") };
    for fmt in ["%d %B %Y", "%d %b %Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(&with_year, fmt) {
            return Ok(d);
        }
    }
    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn dates() {
        let today = d(2026, 11, 18);
        let dmy = DateOrder::DayMonthYear;
        let mdy = DateOrder::MonthDayYear;
        assert_eq!(parse_date("", today, dmy), Ok(today));
        assert_eq!(parse_date("Yesterday", today, dmy), Ok(d(2026, 11, 17)));
        assert_eq!(parse_date("2026-01-05", today, mdy), Ok(d(2026, 1, 5)));
        assert_eq!(parse_date("5/1/2026", today, dmy), Ok(d(2026, 1, 5)));
        assert_eq!(parse_date("5/1/2026", today, mdy), Ok(d(2026, 5, 1)));
        assert_eq!(parse_date("5.1.26", today, dmy), Ok(d(2026, 1, 5)));
        assert_eq!(parse_date("5/1", today, dmy), Ok(d(2026, 1, 5)));
        assert_eq!(parse_date("5/1", today, mdy), Ok(d(2026, 5, 1)));
        assert_eq!(parse_date("2026年11月18日", today, DateOrder::YearMonthDay), Ok(today));
        assert_eq!(parse_date("26-11-18", today, DateOrder::YearMonthDay), Ok(today));
        assert!(parse_date("18/4294967297/2026", today, dmy).is_err());
        assert!(parse_date("31/02/2026", today, dmy).is_err());
        assert!(parse_date("soon", today, dmy).is_err());
        assert!(parse_date("5", today, dmy).is_err());
    }

    #[test]
    fn months() {
        let today = d(2026, 1, 18);
        assert_eq!(parse_month("", today), Ok(d(2026, 1, 1)));
        assert_eq!(parse_month("last", today), Ok(d(2025, 12, 1)));
        assert_eq!(parse_month("2025-07", today), Ok(d(2025, 7, 1)));
        assert_eq!(parse_month("7/2025", today), Ok(d(2025, 7, 1)));
        assert_eq!(parse_month("March", today), Ok(d(2026, 3, 1)));
        assert_eq!(parse_month("mar 2025", today), Ok(d(2025, 3, 1)));
        assert_eq!(parse_month("2025年7月", today), Ok(d(2025, 7, 1)));
        assert!(parse_month("13/2025", today).is_err());
    }

    #[test]
    fn categories_match() {
        let cats = vec![
            Category { id: 22, short: "Entertainment".into() },
            Category { id: 23, short: "Food and Drink".into() },
            Category { id: 24, short: "Fuel".into() },
        ];
        assert_eq!(match_category(&cats, "23").unwrap().id, 23);
        assert_eq!(match_category(&cats, "entertainment").unwrap().id, 22);
        assert_eq!(match_category(&cats, "ent").unwrap().id, 22);
        assert!(match_category(&cats, "f").is_err());
        assert!(match_category(&cats, "99").is_err());
    }
}
