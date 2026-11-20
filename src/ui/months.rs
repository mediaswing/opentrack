//! The Spending and Summary tabs, which show the same month: every entry in it,
//! below the boxes for adding spending, or its totals by category. Moving to
//! another month on one moves the other.

use chrono::{Datelike, Months};
use egui::Ui;

use super::widgets::{RowButton, button_row, dropdown, full_button, heading, text_area};
use super::{App, plural, shortcuts, today};

/// The buttons that change the month.
#[derive(Clone, Copy)]
pub(super) enum MonthStep {
    Previous,
    This,
    Next,
}

impl App {
    /// Moves to another month, and says what is in it.
    pub(super) fn change_month(&mut self, step: MonthStep) {
        self.month = match step {
            MonthStep::Previous => self.month - Months::new(1),
            MonthStep::This => today().with_day(1).expect("every month has a first day"),
            MonthStep::Next => self.month + Months::new(1),
        };
        log::debug!("showing {}", self.month);
        self.entry_to_delete = 0;
        self.refresh();
        self.set_info(format!("Showing {}", self.month_sentence()));
    }

    /// "November 2026: 3 entries, totalling £21.50.", for the status bar.
    fn month_sentence(&self) -> String {
        format!("{}: {}", self.fmt.month_year(self.month), self.month_counts())
    }

    /// "3 entries, totalling £21.50.", under the month's heading.
    fn month_counts(&self) -> String {
        if self.entries.is_empty() {
            return "No spending recorded.".to_owned();
        }
        let total: i64 = self.entries.iter().map(|e| e.pence).sum();
        format!("{}, totalling {}.", plural(self.entries.len(), "entry", "entries"), self.fmt.money(total))
    }

    fn month_controls(&mut self, ui: &mut Ui) {
        let step = button_row(
            ui,
            &[
                RowButton {
                    action: MonthStep::Previous,
                    salt: "previous_month",
                    text: "Previous month",
                    shortcut: Some(&shortcuts::PREVIOUS_MONTH),
                },
                RowButton { action: MonthStep::This, salt: "this_month", text: "This month", shortcut: None },
                RowButton { action: MonthStep::Next, salt: "next_month", text: "Next month", shortcut: Some(&shortcuts::NEXT_MONTH) },
            ],
        );
        if let Some(step) = step {
            self.change_month(step);
        }
        heading(ui, &self.fmt.month_year(self.month), 3);
        ui.label(self.month_counts());
    }

    pub(super) fn spending_tab(&mut self, ui: &mut Ui) {
        heading(ui, "Spending", 2);
        self.add_section(ui);
        self.month_controls(ui);
        if self.entries.is_empty() {
            return;
        }
        let lines: Vec<String> = self.entries.iter().map(|e| self.describe(e)).collect();
        let label = format!("Spending in {}, one entry on each line", self.fmt.month_year(self.month));
        text_area(ui, "entries", &label, &lines.join("\n"));

        heading(ui, "Delete an Entry", 3);
        let options: Vec<String> = self
            .entries
            .iter()
            .map(|e| format!("Entry {}: {}, {}", e.line, e.name, self.fmt.money(e.pence)))
            .collect();
        dropdown(ui, "entry_to_delete", "Entry to delete", &options, &mut self.entry_to_delete);
        let delete = full_button(ui, "delete", "Delete this entry", None);
        if self.wants_focus("delete") {
            delete.request_focus();
        }
        if delete.clicked() {
            self.delete_entry();
        }
    }

    /// Deletes the entry chosen in the dropdown, once the person has agreed.
    fn delete_entry(&mut self) {
        let Some(entry) = self.entries.get(self.entry_to_delete).cloned() else { return };
        let question = format!("Delete this entry? {}", self.describe(&entry));
        if !self.dialogs.confirm("Delete an entry", &question) {
            return self.set_info("Nothing was deleted.");
        }
        match self.db.delete_spending(entry.line) {
            Ok(_) => {
                log::info!("deleted entry {}", entry.line);
                self.refresh();
                // The focus stays on the button, or goes back to the tab if the
                // button has gone with the last entry.
                self.focus_wanted = Some(if self.entries.is_empty() { super::Tab::Spending.name() } else { "delete" });
                self.set_info(format!("Deleted entry {}. {}", entry.line, self.month_sentence()));
            }
            Err(e) => {
                log::error!("could not delete entry {}: {e}", entry.line);
                self.set_error(format!("Entry {} could not be deleted: {e}.", entry.line));
            }
        }
    }

    pub(super) fn summary_tab(&mut self, ui: &mut Ui) {
        heading(ui, "Summary", 2);
        self.month_controls(ui);
        if self.totals.is_empty() {
            return;
        }
        let total: i64 = self.totals.iter().map(|t| t.pence).sum();
        let lines: Vec<String> = self
            .totals
            .iter()
            .map(|t| {
                let share = crate::money::share_percent(t.pence, total)
                    .map_or_else(String::new, |p| format!(", {p} percent of the total"));
                format!("{}: {}, from {}{share}.", t.category, self.fmt.money(t.pence), plural(t.count as usize, "entry", "entries"))
            })
            .collect();
        let label = format!("Spending in {} by category, largest first, one on each line", self.fmt.month_year(self.month));
        text_area(ui, "totals", &label, &lines.join("\n"));
    }
}
