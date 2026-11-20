//! Adding spending, at the top of the Spending tab: what was bought, how
//! much, when, and its category.

use egui::{Key, Ui};

use super::widgets::{dropdown_and_button, heading, text_field};
use super::{App, Tab, shortcuts, today};
use crate::db::Spending;
use crate::locale::parse_date;

impl App {
    pub(super) fn add_section(&mut self, ui: &mut Ui) {
        heading(ui, "Add Spending", 3);
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
        let (_, category, save) = dropdown_and_button(
            ui,
            "category",
            "Category",
            &names,
            &mut self.category_index,
            ("save", "Save spending", Some(&shortcuts::SAVE)),
        );
        if self.wants_focus("category") {
            category.request_focus();
        }
        // Enter in any of the boxes saves, as the button does.
        let entered = [&name, &amount, &date].iter().any(|r| r.lost_focus()) && ui.input(|i| i.key_pressed(Key::Enter));
        if save.clicked() || entered {
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
                if self.tab == Tab::Spending {
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
