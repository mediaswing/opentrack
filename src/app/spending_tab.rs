//! The Spending tab: a form for adding or changing an entry, and the list of
//! entries for the month, which can be searched and filtered.

use super::*;
use crate::db::{Entry, Filter};

pub(super) const NAME_ID: &str = "entry_name";
const DATE_ID: &str = "entry_date";
const PRICE_ID: &str = "entry_price";
pub(super) const FIND_ID: &str = "find_entries";
const PERIOD_FIRST_ID: &str = "period_first";
const PERIOD_LAST_ID: &str = "period_last";

/// A row's id in the list of entries, from its line number.
fn row_id(line: i64) -> Id {
    Id::new(("entry", line))
}

/// An entry's text in the list.
struct RowStrings {
    date: String,
    price: String,
    category: String,
    spoken: String,
}

#[derive(Default)]
pub(super) struct SpendingTab {
    /// The line number of the entry being changed, or `None` for a new one.
    editing: Option<i64>,
    name: String,
    pub(super) date: String,
    price: String,
    /// The chosen category's id.
    pub(super) category: Option<i64>,
    /// List every month, not only the month shown.
    all_months: bool,
    search: String,
    /// Only list this category.
    filter: Option<i64>,
    entries: Vec<Entry>,
    /// Each entry's text as shown and said, made with `entries` rather
    /// than every frame.
    rows: Vec<RowStrings>,
    total: i64,
    /// The entry chosen in the list.
    pub(super) selected: Option<i64>,
    /// A row to give focus to and scroll to.
    focus_row: Option<i64>,
}

impl SpendingTab {
    /// Empties the form for a new entry, dated today, keeping the category.
    pub(super) fn clear_form(&mut self, region: &Region) {
        self.editing = None;
        self.name.clear();
        self.price.clear();
        self.date = region.short_date(money::today());
    }

    /// Writes the form's date the new region's way, if it is a date.
    pub(super) fn rewrite_date(&mut self, old: &Region, new: &Region) {
        if let Ok(date) = old.parse_date(&self.date) {
            self.date = new.short_date(date);
        }
    }

    pub(super) fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    /// Whether anything has been typed into the form for a new entry.
    pub(super) fn has_typing(&self) -> bool {
        !self.name.trim().is_empty() || !self.price.trim().is_empty()
    }

    /// The amounts as the list shows them.
    #[cfg(test)]
    pub(super) fn shown_prices(&self) -> Vec<&str> {
        self.rows.iter().map(|r| r.price.as_str()).collect()
    }
}

impl BudgetApp {
    pub(super) fn reload_entries(&mut self) {
        let filter = Filter {
            period: (!self.spending.all_months).then_some(self.period),
            text: self.spending.search.clone(),
            bucket: self.spending.filter,
        };
        match self.db.entries(&filter) {
            Ok(entries) => {
                self.spending.total = entries.iter().map(|e| e.price).sum();
                if !entries.iter().any(|e| Some(e.line) == self.spending.selected) {
                    self.spending.selected = None;
                }
                self.spending.entries = entries;
            }
            Err(e) => self.show_error(t!("error-read-entries", error = format!("{e:#}"))),
        }
        self.write_rows();
        if self.spending.category.is_none_or(|id| !self.categories.iter().any(|c| c.id == id)) {
            self.spending.category = self.categories.first().map(|c| c.id);
        }
        if self.spending.filter.is_some_and(|id| !self.categories.iter().any(|c| c.id == id)) {
            self.spending.filter = None;
        }
    }

    /// Makes the text of each entry in the list. Called whenever the
    /// entries, categories, region, language or currency symbol change.
    pub(super) fn write_rows(&mut self) {
        let rows = self
            .spending
            .entries
            .iter()
            .map(|entry| {
                let date = self.region.long_date(entry.date);
                let price = self.money(entry.price);
                let category = self.category_name(entry.bucket);
                let spoken =
                    t!("spending-row-spoken", name = &entry.name, amount = &price, category = &category, date = &date);
                RowStrings { date, price, category, spoken }
            })
            .collect();
        self.spending.rows = rows;
    }

    /// Starts a new entry (Ctrl+N).
    pub(super) fn new_entry(&mut self) {
        self.tab = Tab::Spending;
        self.spending.clear_form(&self.region);
        self.focus = Some(Id::new(NAME_ID));
        self.announce(t!("spending-new-announce"));
    }

    pub(super) fn find_entries(&mut self) {
        self.tab = Tab::Spending;
        self.focus = Some(Id::new(FIND_ID));
        self.announce(t!("spending-find-announce"));
    }

    /// Puts an entry in the form to be changed.
    fn edit_entry(&mut self, line: i64) {
        let Some(entry) = self.spending.entries.iter().find(|e| e.line == line).cloned() else { return };
        let form = &mut self.spending;
        form.editing = Some(line);
        form.selected = Some(line);
        form.name = entry.name.clone();
        form.date = self.region.short_date(entry.date);
        form.price = self.region.editable_amount(entry.price);
        form.category = Some(entry.bucket);
        self.tab = Tab::Spending;
        self.focus = Some(Id::new(NAME_ID));
        self.announce(t!("spending-editing-announce", name = &entry.name));
    }

    pub(super) fn edit_selected(&mut self) {
        match self.spending.selected {
            Some(line) if self.tab == Tab::Spending => self.edit_entry(line),
            _ => self.announce(t!("spending-choose-first")),
        }
    }

    /// Sends focus to an entry in the list, or to the form if the entry
    /// isn't listed, so focus is never lost.
    fn focus_entry(&mut self, line: i64) {
        if self.spending.entries.iter().any(|e| e.line == line) {
            self.spending.focus_row = Some(line);
        } else {
            self.focus = Some(Id::new(NAME_ID));
        }
    }

    /// Escape: stops changing an entry, or empties the form.
    pub(super) fn cancel_entry(&mut self) {
        if let Some(line) = self.spending.editing {
            self.spending.clear_form(&self.region);
            self.focus_entry(line);
            self.announce(t!("spending-stopped-editing"));
        } else if self.spending.has_typing() {
            self.spending.clear_form(&self.region);
            self.focus = Some(Id::new(NAME_ID));
            self.announce(t!("spending-cleared"));
        }
    }

    /// Checks the form, then adds the entry or saves the changes (Ctrl+S).
    pub(super) fn save_entry(&mut self) {
        let form = &self.spending;
        let fail = |app: &mut Self, field: &str, message: String| {
            app.focus = Some(Id::new(field));
            app.show_error(message);
        };
        if form.name.trim().is_empty() {
            return fail(self, NAME_ID, t!("entry-name-missing"));
        }
        let date = match self.region.parse_date(&form.date) {
            Ok(date) => date,
            Err(e) => return fail(self, DATE_ID, e),
        };
        let price = match self.parse_amount(&form.price) {
            Ok(price) => price,
            Err(e) => return fail(self, PRICE_ID, e),
        };
        let Some(bucket) = form.category.filter(|id| self.categories.iter().any(|c| c.id == *id)) else {
            return self.show_error(t!("spending-no-category"));
        };
        let entry = Entry { line: form.editing.unwrap_or(0), name: form.name.trim().to_owned(), date, price, bucket };
        let described =
            t!("spending-described", name = &entry.name, amount = self.money(price), category = self.category_name(bucket));
        let result = match form.editing {
            Some(_) => self.db.update_entry(&entry).map(|()| entry.line),
            None => self.db.add_entry(&entry),
        };
        let line = match result {
            Ok(line) => line,
            Err(e) => return self.show_error(t!("error-save-entry", error = format!("{e:#}"))),
        };
        let was_editing = self.spending.editing.is_some();
        self.spending.clear_form(&self.region);
        // The next entry is often on the same day.
        self.spending.date = self.region.short_date(date);
        self.spending.selected = Some(line);
        self.reload();
        let shown = self.region.period_label(self.period);
        let elsewhere = (!self.spending.all_months && !self.period.contains(date)).then(|| match self.period {
            Period::Month(_) => t!("spending-elsewhere", month = Month::of(date).label(), shown = shown),
            Period::Days { .. } => t!("spending-outside", shown = shown),
        });
        let mut message = if was_editing {
            self.focus_entry(line);
            vec![t!("spending-saved", entry = described)]
        } else {
            self.focus = Some(Id::new(NAME_ID));
            vec![t!("spending-added", entry = described)]
        };
        message.extend(elsewhere);
        if !was_editing {
            message.push(t!("spending-next"));
        }
        self.announce(message.join(" "));
    }

    /// Deletes the entry chosen in the list (Ctrl+D or Delete).
    pub(super) fn delete_selected_entry(&mut self) {
        let Some(line) = self.spending.selected else {
            return self.announce(t!("spending-choose-in-list"));
        };
        let Some(index) = self.spending.entries.iter().position(|e| e.line == line) else { return };
        let entry = self.spending.entries[index].clone();
        let described = t!(
            "spending-described-dated",
            name = &entry.name,
            amount = self.money(entry.price),
            date = self.region.long_date(entry.date)
        );
        if self.settings.confirm_delete
            && !self.confirm(&t!("spending-delete-title"), &t!("spending-delete-question", entry = &described))
        {
            self.focus_entry(line);
            return self.announce(t!("nothing-deleted"));
        }
        if let Err(e) = self.db.delete_entry(line) {
            return self.show_error(t!("error-delete-entry", error = format!("{e:#}")));
        }
        if self.spending.editing == Some(line) {
            self.spending.clear_form(&self.region);
        }
        // Focus moves to the next entry, or the one before if it was last.
        let entries = &self.spending.entries;
        let next = entries.get(index + 1).or_else(|| index.checked_sub(1).and_then(|i| entries.get(i))).map(|e| e.line);
        self.spending.selected = next;
        self.reload();
        match next {
            Some(next) => self.spending.focus_row = Some(next),
            None => self.focus = Some(Id::new(NAME_ID)),
        }
        self.announce(t!("spending-deleted", entry = described));
    }

    pub(super) fn spending_tab(&mut self, ui: &mut Ui) {
        self.entry_form(ui);
        ui.add_space(8.0);
        ui.separator();
        self.period_controls(ui);
        self.entry_list(ui);
    }

    fn entry_form(&mut self, ui: &mut Ui) {
        let editing = self.spending.editing;
        match editing {
            Some(line) => heading(ui, &t!("spending-edit-heading", line = line)),
            None => heading(ui, &t!("spending-add-heading")),
        }
        let form = &mut self.spending;
        let name = text_field(ui, Id::new(NAME_ID), &t!("spending-name-label"), &mut form.name, &t!("spending-name-hint"));
        if form.name.chars().count() > crate::db::MAX_NAME_CHARS {
            form.name = form.name.chars().take(crate::db::MAX_NAME_CHARS).collect();
        }
        let date_label = t!("spending-date-label", pattern = self.region.date_pattern());
        let date_hint = t!("hint-such-as", example = self.region.example_date());
        let date = text_field(ui, Id::new(DATE_ID), &date_label, &mut form.date, &date_hint);
        let price_hint = t!("hint-such-as", example = self.region.example_amount());
        let price = text_field(ui, Id::new(PRICE_ID), &t!("spending-amount-label"), &mut form.price, &price_hint);
        // Enter in any box of the form saves, as in a dialog.
        let entered = [name, date, price].iter().any(|r| r.lost_focus()) && ui.input(|i| i.key_pressed(Key::Enter));

        let names: Vec<String> = self.categories.iter().map(|c| c.short.clone()).collect();
        let mut index = self.categories.iter().position(|c| Some(c.id) == self.spending.category).unwrap_or(0);
        if dropdown(ui, "entry_category", &t!("category-label"), &names, &mut index, !names.is_empty()) {
            self.spending.category = self.categories.get(index).map(|c| c.id);
        }

        let save = if editing.is_some() { t!("save-changes") } else { t!("spending-add-button") };
        let save = format!("{save}{}", self.keymap.hint(Action::Save));
        let cancel = if editing.is_some() { t!("spending-cancel-editing") } else { t!("spending-clear-form") };
        match button_row(ui, &[(&save, true), (&cancel, editing.is_some() || self.spending.has_typing())]) {
            Some(0) => self.save_entry(),
            Some(_) => self.cancel_entry(),
            None => {}
        }
        if entered {
            self.save_entry();
        }
    }

    /// Which dates the Spending and Summary tabs show: one month, or the
    /// days typed in From and To, which start as this financial year. Then
    /// buttons for the dates before, this month, and the dates after.
    pub(super) fn period_controls(&mut self, ui: &mut Ui) {
        let kinds = [t!("period-one-month"), t!("period-choose-dates")];
        let days = matches!(self.period, Period::Days { .. });
        let mut which = usize::from(days);
        if dropdown(ui, "period_kind", &t!("period-label"), &kinds, &mut which, true) && (which == 1) != days {
            let today = money::today();
            if which == 1 {
                self.change_period(self.year_start().year_of(today));
            } else {
                let month = if self.period.contains(today) { Month::of(today) } else { Month::of(self.period.first_day()) };
                self.change_period(Period::Month(month));
            }
        }
        if days {
            let hint = t!("hint-such-as", example = self.region.example_date());
            let (first_label, last_label) = (t!("period-first-label"), t!("period-last-label"));
            let mut entered = false;
            ui.columns(2, |cols| {
                let first = text_field(&mut cols[0], Id::new(PERIOD_FIRST_ID), &first_label, &mut self.period_first, &hint);
                let last = text_field(&mut cols[1], Id::new(PERIOD_LAST_ID), &last_label, &mut self.period_last, &hint);
                entered = (first.lost_focus() || last.lost_focus()) && cols[0].input(|i| i.key_pressed(Key::Enter));
            });
            if full_button(ui, &t!("period-show-dates"), true).clicked() || entered {
                self.show_typed_dates();
            }
        }

        let (previous, next) =
            if days { (t!("period-previous"), t!("period-next")) } else { (t!("month-previous"), t!("month-next")) };
        let previous = format!("{previous}{}", self.keymap.hint(Action::PreviousMonth));
        let this = format!("{}{}", t!("month-this"), self.keymap.hint(Action::ThisMonth));
        let next = format!("{next}{}", self.keymap.hint(Action::NextMonth));
        let current = self.period == Period::Month(Month::current());
        match button_row(ui, &[(&previous, true), (&this, !current), (&next, true)]) {
            Some(0) => self.change_period(self.period.previous()),
            Some(1) => self.change_period(Period::Month(Month::current())),
            Some(_) => self.change_period(self.period.next()),
            None => {}
        }
    }

    /// Puts the dates shown in the From and To boxes, the region's way.
    pub(super) fn write_period_dates(&mut self) {
        if let Period::Days { first, last } = self.period {
            self.period_first = self.region.short_date(first);
            self.period_last = self.region.short_date(last);
        }
    }

    /// Shows the dates typed in From and To.
    pub(super) fn show_typed_dates(&mut self) {
        let parsed = [(&self.period_first, PERIOD_FIRST_ID), (&self.period_last, PERIOD_LAST_ID)]
            .map(|(text, id)| self.region.parse_date(text).map_err(|e| (e, id)));
        let result = match parsed {
            [Ok(first), Ok(last)] if last < first => Err((t!("period-backwards"), PERIOD_LAST_ID)),
            [Ok(first), Ok(last)] => Ok(Period::Days { first, last }),
            [Err(e), _] | [_, Err(e)] => Err(e),
        };
        match result {
            Ok(period) => self.change_period(period),
            Err((message, id)) => {
                self.focus = Some(Id::new(id));
                self.show_error(message);
            }
        }
    }

    fn entry_list(&mut self, ui: &mut Ui) {
        let title = if self.spending.all_months {
            t!("spending-list-all-heading")
        } else {
            t!("spending-list-month-heading", month = self.region.period_label(self.period))
        };
        heading(ui, &title);

        let only = t!("spending-show-only", month = self.region.period_label(self.period));
        let shown = [only, t!("spending-show-every")];
        let mut which = usize::from(self.spending.all_months);
        let mut changed = dropdown(ui, "entry_months", &t!("spending-show-label"), &shown, &mut which, true);
        self.spending.all_months = which == 1;

        let mut filters = vec![t!("spending-every-category")];
        filters.extend(self.categories.iter().map(|c| c.short.clone()));
        let mut index =
            self.spending.filter.and_then(|id| self.categories.iter().position(|c| c.id == id)).map_or(0, |i| i + 1);
        if dropdown(ui, "entry_filter", &t!("spending-filter-label"), &filters, &mut index, true) {
            self.spending.filter = index.checked_sub(1).and_then(|i| self.categories.get(i)).map(|c| c.id);
            changed = true;
        }
        let find = format!("{}{}", t!("spending-find-label"), self.keymap.hint(Action::Find));
        if text_field(ui, Id::new(FIND_ID), &find, &mut self.spending.search, &t!("spending-find-hint")).changed() {
            changed = true;
        }
        if changed {
            self.reload_entries();
        }

        let count = self.spending.entries.len();
        let summary = match count {
            0 if !self.spending.search.trim().is_empty() => t!("spending-none-match"),
            0 => t!("spending-none-yet"),
            n => t!("spending-total", count = n, amount = self.money(self.spending.total)),
        };
        note(ui, &summary);

        let mut pressed = None;
        let mut focused_index = None;
        for (i, (entry, row)) in self.spending.entries.iter().zip(&self.spending.rows).enumerate() {
            let text = RowText {
                date: &row.date,
                name: &entry.name,
                category: &row.category,
                price: &row.price,
                spoken: &row.spoken,
            };
            let selected = self.spending.selected == Some(entry.line);
            let resp = entry_row(ui, row_id(entry.line), &text, selected);
            if self.spending.focus_row == Some(entry.line) {
                resp.request_focus();
                resp.scroll_to_me(None);
            }
            if resp.has_focus() {
                focused_index = Some(i);
            }
            if resp.clicked() {
                pressed = Some(entry.line);
            }
        }
        self.spending.focus_row = None;
        if let Some(i) = focused_index {
            self.spending.selected = Some(self.spending.entries[i].line);
            self.list_keys(ui, i);
        }
        if let Some(line) = pressed {
            self.edit_entry(line);
        }

        let has = self.spending.selected.is_some();
        let edit = format!("{}{}", t!("spending-edit-chosen"), self.keymap.hint(Action::Edit));
        let delete = format!("{}{}", t!("spending-delete-chosen"), self.keymap.hint(Action::Delete));
        match button_row(ui, &[(&edit, has), (&delete, has)]) {
            Some(0) => self.edit_selected(),
            Some(_) => self.delete_selected_entry(),
            None => {}
        }
    }

    /// While an entry in the list has focus: Up and Down move to the one
    /// before or after, Page Up and Page Down ten at a time, Home and End to
    /// the first and last, and Delete deletes it.
    fn list_keys(&mut self, ui: &mut Ui, index: usize) {
        let last = self.spending.entries.len().saturating_sub(1);
        ui.memory_mut(|m| {
            let focused = row_id(self.spending.entries[index].line);
            m.set_focus_lock_filter(focused, EventFilter { vertical_arrows: true, ..Default::default() });
        });
        let (to, delete) = ui.input_mut(|i| {
            let to = if consume_exact(i, Modifiers::NONE, Key::ArrowDown) {
                Some((index + 1).min(last))
            } else if consume_exact(i, Modifiers::NONE, Key::ArrowUp) {
                Some(index.saturating_sub(1))
            } else if consume_exact(i, Modifiers::NONE, Key::PageDown) {
                Some((index + 10).min(last))
            } else if consume_exact(i, Modifiers::NONE, Key::PageUp) {
                Some(index.saturating_sub(10))
            } else if consume_exact(i, Modifiers::NONE, Key::Home) {
                Some(0)
            } else if consume_exact(i, Modifiers::NONE, Key::End) {
                Some(last)
            } else {
                None
            };
            (to, consume_exact(i, Modifiers::NONE, Key::Delete))
        });
        if let Some(to) = to.filter(|to| *to != index) {
            let line = self.spending.entries[to].line;
            self.spending.selected = Some(line);
            self.spending.focus_row = Some(line);
            ui.ctx().request_repaint();
        }
        if delete {
            self.delete_selected_entry();
        }
    }
}
