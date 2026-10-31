//! The Categories tab: renaming categories, setting their monthly budgets,
//! deleting them and adding new ones.

use super::*;

const NAME_ID: &str = "category_name";
const BUDGET_ID: &str = "category_budget";
pub(super) const NEW_NAME_ID: &str = "new_category_name";
const NEW_BUDGET_ID: &str = "new_category_budget";

#[derive(Default)]
pub(super) struct CategoryForm {
    /// The category being changed.
    selected: Option<i64>,
    /// The category whose name and budget are in the boxes.
    loaded: Option<i64>,
    name: String,
    budget: String,
    /// Where a deleted category's entries go.
    move_to: Option<i64>,
    new_name: String,
    new_budget: String,
}

impl CategoryForm {
    /// Fills the boxes again at the next `sync`, such as after the region
    /// changes how budgets are written.
    pub(super) fn reload(&mut self) {
        self.loaded = None;
    }

    /// Keeps the choices pointing at categories that exist, and fills the
    /// boxes when another category is chosen.
    pub(super) fn sync(&mut self, categories: &[Category], region: &Region) {
        let exists = |id: Option<i64>| id.is_some_and(|id| categories.iter().any(|c| c.id == id));
        if !exists(self.selected) {
            self.selected = categories.first().map(|c| c.id);
        }
        if !exists(self.move_to) || self.move_to == self.selected {
            self.move_to = categories.iter().find(|c| Some(c.id) != self.selected).map(|c| c.id);
        }
        if self.loaded != self.selected {
            self.loaded = self.selected;
            let category = categories.iter().find(|c| Some(c.id) == self.selected);
            self.name = category.map(|c| c.short.clone()).unwrap_or_default();
            self.budget = category.and_then(|c| c.budget).map(|b| region.editable_amount(b)).unwrap_or_default();
        }
    }
}

impl BudgetApp {
    /// A budget as typed: empty for none.
    fn parse_budget(&self, text: &str) -> Result<Option<i64>, String> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        match self.parse_amount(text)? {
            pence if pence < 0 => Err(t!("budget-negative")),
            pence => Ok(Some(pence)),
        }
    }

    /// Ctrl+N on the Categories tab.
    pub(super) fn new_category(&mut self) {
        self.focus = Some(Id::new(NEW_NAME_ID));
        self.announce(t!("categories-new-announce"));
    }

    /// Whether focus is in the boxes for adding a category, so Ctrl+S adds
    /// one rather than saving the chosen category.
    pub(super) fn adding_category(&self, ctx: &egui::Context) -> bool {
        let focused = ctx.memory(|m| m.focused());
        focused == Some(Id::new(NEW_NAME_ID)) || focused == Some(Id::new(NEW_BUDGET_ID))
    }

    pub(super) fn save_category(&mut self) {
        let Some(id) = self.category_form.selected else {
            return self.announce(t!("categories-none-yet"));
        };
        let budget = match self.parse_budget(&self.category_form.budget) {
            Ok(budget) => budget,
            Err(e) => {
                self.focus = Some(Id::new(BUDGET_ID));
                return self.show_error(e);
            }
        };
        if let Err(e) = self.db.update_category(id, &self.category_form.name, budget) {
            self.focus = Some(Id::new(NAME_ID));
            return self.show_error(format!("{e:#}"));
        }
        // Fill the boxes again with the name as saved.
        self.category_form.loaded = None;
        self.reload();
        let name = self.category_name(id);
        match budget {
            Some(b) => self.announce(t!("categories-saved-budget", name = name, amount = self.money(b))),
            None => self.announce(t!("categories-saved-no-budget", name = name)),
        }
    }

    pub(super) fn add_category(&mut self) {
        let budget = match self.parse_budget(&self.category_form.new_budget) {
            Ok(budget) => budget,
            Err(e) => {
                self.focus = Some(Id::new(NEW_BUDGET_ID));
                return self.show_error(e);
            }
        };
        match self.db.add_category(&self.category_form.new_name, budget) {
            Ok(id) => {
                self.category_form.new_name.clear();
                self.category_form.new_budget.clear();
                self.reload();
                self.focus = Some(Id::new(NEW_NAME_ID));
                let name = self.category_name(id);
                self.announce(t!("categories-added", name = name));
            }
            Err(e) => {
                self.focus = Some(Id::new(NEW_NAME_ID));
                self.show_error(format!("{e:#}"));
            }
        }
    }

    pub(super) fn delete_category(&mut self) {
        let Some(id) = self.category_form.selected else {
            return self.announce(t!("categories-none-to-delete"));
        };
        let name = self.category_name(id);
        let count = self.db.entry_count(id).unwrap_or(0);
        let move_to = self.category_form.move_to.filter(|_| count > 0);
        let question = match move_to {
            _ if count == 0 => t!("categories-delete-question", name = &name),
            Some(to) => t!("categories-delete-move-question", name = &name, count = count, to = self.category_name(to)),
            None => return self.show_error(t!("categories-has-entries", name = &name)),
        };
        if !self.confirm(&t!("categories-delete-title"), &question) {
            return self.announce(t!("nothing-deleted"));
        }
        match self.db.delete_category(id, move_to) {
            Ok(()) => {
                self.category_form.selected = None;
                self.reload();
                self.announce(t!("categories-deleted", name = &name));
            }
            Err(e) => self.show_error(t!("error-delete-category", name = &name, error = format!("{e:#}"))),
        }
    }

    pub(super) fn categories_tab(&mut self, ui: &mut Ui) {
        heading(ui, &t!("categories-change-heading"));
        if self.categories.is_empty() {
            note(ui, &t!("categories-none-add-below"));
        } else {
            let names: Vec<String> = self
                .categories
                .iter()
                .map(|c| match c.budget {
                    Some(b) => t!("categories-with-budget", name = &c.short, amount = self.money(b)),
                    None => c.short.clone(),
                })
                .collect();
            let mut index =
                self.categories.iter().position(|c| Some(c.id) == self.category_form.selected).unwrap_or(0);
            if dropdown(ui, "category_choice", &t!("category-label"), &names, &mut index, true) {
                self.category_form.selected = self.categories.get(index).map(|c| c.id);
                self.category_form.sync(&self.categories, &self.region);
            }
            let form = &mut self.category_form;
            let name = text_field(ui, Id::new(NAME_ID), &t!("categories-name-label"), &mut form.name, &t!("categories-name-hint"));
            let budget_hint = t!("hint-such-as", example = "200");
            let budget = text_field(ui, Id::new(BUDGET_ID), &t!("categories-budget-label"), &mut form.budget, &budget_hint);
            let entered = (name.lost_focus() || budget.lost_focus()) && ui.input(|i| i.key_pressed(Key::Enter));

            let count = form.selected.and_then(|id| self.entry_counts.get(&id).copied()).unwrap_or(0);
            note(ui, &t!("categories-entry-count", count = count));
            if count > 0 && self.categories.len() > 1 {
                let others: Vec<&Category> =
                    self.categories.iter().filter(|c| Some(c.id) != self.category_form.selected).collect();
                let names: Vec<String> = others.iter().map(|c| c.short.clone()).collect();
                let mut index = others.iter().position(|c| Some(c.id) == self.category_form.move_to).unwrap_or(0);
                if dropdown(ui, "category_move_to", &t!("categories-move-label"), &names, &mut index, true) {
                    self.category_form.move_to = others.get(index).map(|c| c.id);
                }
            }
            let save = format!("{}{}", t!("save-changes"), self.keymap.hint(Action::Save));
            let delete = format!("{}{}", t!("categories-delete-button"), self.keymap.hint(Action::Delete));
            match button_row(ui, &[(&save, true), (&delete, true)]) {
                Some(0) => self.save_category(),
                Some(_) => self.delete_category(),
                None if entered => self.save_category(),
                None => {}
            }
        }

        ui.add_space(8.0);
        ui.separator();
        heading(ui, &t!("categories-add-heading"));
        let form = &mut self.category_form;
        let name_label = format!("{}{}", t!("categories-new-name-label"), self.keymap.hint(Action::New));
        let name = text_field(ui, Id::new(NEW_NAME_ID), &name_label, &mut form.new_name, &t!("categories-new-name-hint"));
        let budget_label = t!("categories-new-budget-label");
        let budget_hint = t!("hint-such-as", example = "50");
        let budget = text_field(ui, Id::new(NEW_BUDGET_ID), &budget_label, &mut form.new_budget, &budget_hint);
        let entered = (name.lost_focus() || budget.lost_focus()) && ui.input(|i| i.key_pressed(Key::Enter));
        if full_button(ui, &t!("categories-add-button"), true).clicked() || entered {
            self.add_category();
        }
    }
}
