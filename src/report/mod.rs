//! Spending reports, as a PDF or a Word document. Both are made from the same
//! outline of headings, paragraphs and tables, and both are built to be read
//! with a screen reader: headings are headings, tables have header rows, and
//! the document has a title and a language.

mod docx;
mod pdf;
mod zip;

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};

use crate::db::Spending;
use crate::locale::LocaleFormat;
use crate::money;

pub use docx::docx;
pub use pdf::pdf;

/// The spending a report covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    /// The month starting on this day.
    Month(NaiveDate),
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// The report's title, a level 1 heading.
    Title(String),
    /// A section's heading, level 2.
    Heading(String),
    Paragraph(String),
    Table(Table),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub title: &'static str,
    /// Its share of the page's width.
    pub width: f32,
    /// Numbers line up on the right.
    pub numeric: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// What the table shows, for screen readers.
    pub summary: String,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// The document's title, as its file's properties give it.
    pub title: String,
    /// The language the words are in, such as en-GB.
    pub language: String,
    pub blocks: Vec<Block>,
}

const fn column(title: &'static str, width: f32, numeric: bool) -> Column {
    Column { title, width, numeric }
}

/// "1 entry", "2 entries".
fn entries_count(n: usize) -> String {
    format!("{n} {}", if n == 1 { "entry" } else { "entries" })
}

/// Lays out a report of `entries`, which are oldest first.
pub fn build(entries: &[Spending], period: Period, fmt: &LocaleFormat, today: NaiveDate) -> Report {
    let total: i64 = entries.iter().map(|e| e.pence).sum();
    let (title, covers) = match period {
        Period::Month(month) => {
            let name = fmt.month_year(month);
            (format!("Spending Report: {name}"), format!("Spending in {name}."))
        }
        Period::All => {
            let dates = (entries.iter().map(|e| e.date).min(), entries.iter().map(|e| e.date).max());
            let covers = match dates {
                (Some(first), Some(last)) if first != last => {
                    format!("All spending, from {} to {}.", fmt.long_date(first), fmt.long_date(last))
                }
                (Some(only), _) => format!("All spending, on {}.", fmt.long_date(only)),
                _ => "All spending.".to_owned(),
            };
            ("Spending Report: All Spending".to_owned(), covers)
        }
    };
    let mut blocks = vec![Block::Title(title.clone()), Block::Paragraph(covers)];
    if entries.is_empty() {
        blocks.push(Block::Paragraph("No spending recorded.".to_owned()));
    } else {
        blocks.push(Block::Paragraph(format!("{}, totalling {}.", entries_count(entries.len()), fmt.money(total))));
        blocks.push(Block::Heading("By Category".to_owned()));
        blocks.push(Block::Table(by_category(entries, total, fmt)));
        if period == Period::All && entries.iter().any(|e| month_of(e) != month_of(&entries[0])) {
            blocks.push(Block::Heading("By Month".to_owned()));
            blocks.push(Block::Table(by_month(entries, fmt)));
        }
        blocks.push(Block::Heading("Every Entry".to_owned()));
        blocks.push(Block::Table(Table {
            summary: "Every entry, oldest first: its date, what was bought, its category and amount".to_owned(),
            columns: vec![column("Date", 0.17, false), column("What You Bought", 0.43, false), column("Category", 0.24, false), column("Amount", 0.16, true)],
            rows: entries
                .iter()
                .map(|e| vec![fmt.short_date(e.date), e.name.clone(), e.category.clone(), fmt.money(e.pence)])
                .collect(),
        }));
    }
    blocks.push(Block::Paragraph(format!(
        "Made by {} {} on {}.",
        crate::APP_TITLE,
        env!("CARGO_PKG_VERSION"),
        fmt.long_date(today)
    )));
    Report { title, language: language_tag(fmt), blocks }
}

fn month_of(e: &Spending) -> (i32, u32) {
    (e.date.year(), e.date.month())
}

/// Totals by category, largest first, as on the Summary tab.
fn by_category(entries: &[Spending], total: i64, fmt: &LocaleFormat) -> Table {
    let mut sums: BTreeMap<&str, (i64, usize)> = BTreeMap::new();
    for e in entries {
        let sum = sums.entry(e.category.as_str()).or_default();
        sum.0 += e.pence;
        sum.1 += 1;
    }
    let mut sums: Vec<_> = sums.into_iter().collect();
    sums.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    Table {
        summary: "Spending by category, largest first: the number of entries, the amount and its share of the total".to_owned(),
        columns: vec![column("Category", 0.43, false), column("Entries", 0.17, true), column("Amount", 0.24, true), column("Share", 0.16, true)],
        rows: sums
            .into_iter()
            .map(|(category, (pence, count))| {
                let share = money::share_percent(pence, total).map_or_else(String::new, |p| format!("{p}%"));
                vec![category.to_owned(), count.to_string(), fmt.money(pence), share]
            })
            .collect(),
    }
}

/// Totals by month, oldest first.
fn by_month(entries: &[Spending], fmt: &LocaleFormat) -> Table {
    let mut sums: BTreeMap<(i32, u32), (i64, usize)> = BTreeMap::new();
    for e in entries {
        let sum = sums.entry(month_of(e)).or_default();
        sum.0 += e.pence;
        sum.1 += 1;
    }
    Table {
        summary: "Spending by month, oldest first: the number of entries and the amount".to_owned(),
        columns: vec![column("Month", 0.5, false), column("Entries", 0.2, true), column("Amount", 0.3, true)],
        rows: sums
            .into_iter()
            .map(|((year, month), (pence, count))| {
                let first = NaiveDate::from_ymd_opt(year, month, 1).expect("a month from a real date");
                vec![fmt.month_year(first), count.to_string(), fmt.money(pence)]
            })
            .collect(),
    }
}

/// The report's words are English, so it is English as written where the
/// locale is English, and British English otherwise.
fn language_tag(fmt: &LocaleFormat) -> String {
    let locale = fmt.locale.to_string();
    match locale.split_once('_') {
        Some(("en", region)) if region.len() == 2 => format!("en-{region}"),
        _ => "en-GB".to_owned(),
    }
}

/// Text for XML: escaped, and without the control characters XML forbids.
fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use chrono::Locale;

    pub fn gb() -> LocaleFormat {
        LocaleFormat::new(Locale::en_GB, "test")
    }

    pub fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    pub fn sample() -> Vec<Spending> {
        let e = |line, date, name: &str, pence, category: &str| Spending {
            line,
            date: d(date),
            name: name.to_owned(),
            pence,
            category: category.to_owned(),
        };
        vec![
            e(1, "2026-10-30", "Cinema & <popcorn>", 1250, "Entertainment"),
            e(2, "2026-11-02", "Pizza", 900, "Food and Drink"),
            e(3, "2026-11-18", "Café crème £ € Łódź", 350, "Food and Drink"),
            e(4, "2026-11-19", "Refund", -200, "Entertainment"),
        ]
    }

    #[test]
    fn a_report_has_totals_by_category_and_month_and_every_entry() {
        let report = build(&sample(), Period::All, &gb(), d("2026-11-20"));
        assert_eq!(report.title, "Spending Report: All Spending");
        assert_eq!(report.language, "en-GB");
        let headings: Vec<&str> = report
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Title(t) | Block::Heading(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(headings, ["Spending Report: All Spending", "By Category", "By Month", "Every Entry"]);
        assert_eq!(report.blocks[1], Block::Paragraph("All spending, from Friday 30 October 2026 to Thursday 19 November 2026.".to_owned()));
        assert_eq!(report.blocks[2], Block::Paragraph("4 entries, totalling £23.00.".to_owned()));
        let Block::Table(categories) = &report.blocks[4] else { panic!() };
        assert_eq!(categories.rows[0], ["Food and Drink", "2", "£12.50", "54%"]);
        assert_eq!(categories.rows[1], ["Entertainment", "2", "£10.50", "46%"]);
        let Block::Table(months) = &report.blocks[6] else { panic!() };
        assert_eq!(months.rows, [["October 2026", "1", "£12.50"], ["November 2026", "3", "£10.50"]]);
        let Block::Table(every) = &report.blocks[8] else { panic!() };
        assert_eq!(every.rows[3], ["19/11/2026", "Refund", "Entertainment", "minus £2.00"]);
        assert!(matches!(report.blocks.last(), Some(Block::Paragraph(p)) if p.starts_with("Made by Open Budget Tracking")));
    }

    #[test]
    fn a_month_report_has_no_months_table() {
        let month = d("2026-11-01");
        let entries: Vec<_> = sample().into_iter().filter(|e| e.date >= month).collect();
        let report = build(&entries, Period::Month(month), &gb(), d("2026-11-20"));
        assert_eq!(report.title, "Spending Report: November 2026");
        assert!(!report.blocks.contains(&Block::Heading("By Month".to_owned())));
        let empty = build(&[], Period::Month(month), &gb(), d("2026-11-20"));
        assert!(empty.blocks.contains(&Block::Paragraph("No spending recorded.".to_owned())));
    }

    #[test]
    fn the_language_follows_english_locales() {
        assert_eq!(language_tag(&LocaleFormat::new(Locale::en_US, "test")), "en-US");
        assert_eq!(language_tag(&LocaleFormat::new(Locale::de_DE, "test")), "en-GB");
    }
}
