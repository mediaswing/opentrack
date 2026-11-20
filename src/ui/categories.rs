//! The Categories tab: the categories there are, and adding a new one.

use egui::{Key, Ui};

use super::widgets::{full_button, heading, text_area, text_field};
use super::{App, plural, shortcuts};

impl App {
    pub(super) fn categories_tab(&mut self, ui: &mut Ui) {
        heading(ui, "Categories", 2);
        ui.label(format!(
            "There are {}. Each piece of spending goes in one, which you choose on the Spending tab.",
            plural(self.categories.len(), "category", "categories")
        ));
        let names: Vec<&str> = self.categories.iter().map(|c| c.short.as_str()).collect();
        text_area(ui, "categories", "Categories, one on each line", &names.join("\n"));

        heading(ui, "New Category", 3);
        let field = text_field(ui, "Name of the new category, up to 32 characters", "new_category", &mut self.new_category);
        if self.wants_focus("new_category") {
            field.request_focus();
        }
        let entered = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if full_button(ui, "add_category", "Add category", Some(&shortcuts::SAVE)).clicked() || entered {
            self.add_category();
        }
    }

    pub(super) fn add_category(&mut self) {
        let name = self.new_category.trim().to_owned();
        // The focus stays in, or goes back to, the box, ready for another.
        self.focus_wanted = Some("new_category");
        if name.is_empty() {
            return self.set_error("Type a name for the new category first.");
        }
        if name.chars().count() > 32 {
            return self.set_error("A category's name can be up to 32 characters long.");
        }
        match self.db.add_category(&name) {
            Ok(id) => {
                log::info!("added category {id}");
                self.new_category.clear();
                self.refresh();
                self.set_info(format!("Added the category {name}. You can choose it on the Spending tab."));
            }
            Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == rusqlite::ErrorCode::ConstraintViolation => {
                self.set_error(format!("There is already a category called {name}."));
            }
            Err(e) => {
                log::error!("could not add a category: {e}");
                self.set_error(format!("The category could not be added: {e}."));
            }
        }
    }
}
