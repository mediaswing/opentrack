//! The Add Spending tab: what was bought, how much, when, and its category.

use chrono::{Datelike, Duration, NaiveDate};
use egui::{Key, Ui};

use super::widgets::{dropdown, full_button, heading, text_field};
use super::{App, Tab, shortcuts, today};
use crate::db::Spending;
use crate::locale::DateOrder;

impl App {
    pub(super) fn add_tab(&mut self, ui: &mut Ui) {
        heading(ui, "Add Spending", 2);
        let name = text_field(ui, "What you bought", "name", &mut self.name_input);
        if self.wants_focus("name") {
            name.request_focus();
        }
        let amount_label = format!("Amount, for example {}", self.fmt.example_amount());
        let amount = text_field(ui, &amount_label, "amount", &mut self.amount_input);
        if self.wants_focus("amount") {
            amount.request_focus();
        }
        let date_label = format!("Date, such as {}; leave empty for today", self.fmt.short_date(today()));
        let date = text_field(ui, &date_label, "date", &mut self.date_input);
        if self.wants_focus("date") {
            date.request_focus();
        }
        let names: Vec<String> = self.categories.iter().map(|c| c.short.clone()).collect();
        let (_, category) = dropdown(ui, "category", "Category", &names, &mut self.category_index);
        if self.wants_focus("category") {
            category.request_focus();
        }
        // Enter in any of the boxes saves, as the button does.
        let entered = [&name, &amount, &date].iter().any(|r| r.lost_focus()) && ui.input(|i| i.key_pressed(Key::Enter));
        if full_button(ui, "save", "Save spending", Some(&shortcuts::SAVE)).clicked() || entered {
            self.save_spending();
        }
    }

    /// Checks what was typed and saves it. A mistake is announced, and the
    /// focus goes to the box that needs changing.
    pub(super) fn save_spending(&mut self) {
        let name = self.name_input.trim().to_owned();
        if name.is_empty() {
            self.focus_wanted = Some("name");
            return self.set_error("Type what you bought first.");
        }
        if name.chars().count() > 160 {
            self.focus_wanted = Some("name");
            return self.set_error("What you bought can be up to 160 characters long.");
        }
        if self.amount_input.trim().is_empty() {
            self.focus_wanted = Some("amount");
            return self.set_error("Type how much it was.");
        }
        let pence = match self.fmt.parse_money(&self.amount_input) {
            Ok(p) => p,
            Err(msg) => {
                self.focus_wanted = Some("amount");
                return self.set_error(msg);
            }
        };
        let Ok(date) = parse_date(&self.date_input, today(), self.fmt.date_order) else {
            self.focus_wanted = Some("date");
            let example = self.fmt.short_date(today());
            return self.set_error(format!(
                "\"{}\" is not a date I understand. Type a date such as {example}, or yesterday, or leave it empty for today.",
                self.date_input.trim()
            ));
        };
        let Some(category) = self.categories.get(self.category_index).cloned() else {
            return self.set_error("There are no categories. Add one on the Categories tab first.");
        };
        match self.db.add_spending(&name, date, pence, category.id) {
            Ok(line) => {
                log::info!("added entry {line} in category {}", category.id);
                let entry = Spending { line, name, date, pence, category: category.short };
                let saved = format!("Saved. {}", self.describe(&entry));
                self.name_input.clear();
                self.amount_input.clear();
                self.date_input.clear();
                self.refresh();
                self.set_info(saved);
                // Ready for the next one.
                if self.tab == Tab::Add {
                    self.focus_wanted = Some("name");
                }
            }
            Err(e) => {
                log::error!("could not add spending: {e}");
                self.set_error(format!("The spending could not be saved: {e}."));
            }
        }
    }
}

/// Parse a typed date. Numbers may be separated by any non-digit character. A
/// four-digit first number means year first; otherwise the locale's order is
/// used. A missing year means the current year, and empty means today.
pub(super) fn parse_date(text: &str, today: NaiveDate, order: DateOrder) -> Result<NaiveDate, ()> {
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
}
