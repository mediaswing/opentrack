use std::collections::BTreeSet;

use egui::RichText;
use egui_extras::{Column, DatePickerButton, TableBuilder};
use jiff::civil::Date;

use super::{App, month_key, today};
use crate::db::Spending;
use crate::money;

pub struct State {
    /// `Some(line)` while editing an existing row.
    editing: Option<i64>,
    name: String,
    date: Date,
    amount: String,
    bucket: Option<i64>,
    /// The team to share it with, if any. Private by default.
    team: Option<i64>,
    // Filters
    month: Option<(i16, i8)>,
    category: Option<i64>,
    search: String,
    confirm_delete: Option<Spending>,
    /// Move the keyboard focus to the description or search field next frame.
    focus_name: bool,
    focus_search: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            editing: None,
            name: String::new(),
            date: today(),
            amount: String::new(),
            bucket: None,
            team: None,
            month: Some(month_key(today())),
            category: None,
            search: String::new(),
            confirm_delete: None,
            focus_name: false,
            focus_search: false,
        }
    }
}

impl State {
    fn clear_form(&mut self) {
        self.editing = None;
        self.name.clear();
        self.amount.clear();
        self.date = today();
        self.team = None;
    }

    /// An empty form for a new transaction, ready to type into.
    pub(super) fn start_new(&mut self) {
        self.clear_form();
        self.focus_name = true;
    }

    pub(super) fn focus_search(&mut self) {
        self.focus_search = true;
    }

    /// Stop editing a transaction, discarding the changes.
    pub(super) fn cancel_edit(&mut self) {
        if self.editing.is_some() {
            self.clear_form();
        }
    }

    /// Whether the form has anything worth submitting.
    pub(super) fn has_input(&self) -> bool {
        self.editing.is_some() || !self.name.trim().is_empty() || !self.amount.trim().is_empty()
    }

    /// Clear the month filter, so transactions from any month are listed.
    pub(super) fn show_all_months(&mut self) {
        self.month = None;
    }

    /// Whether the delete confirmation is open.
    pub(super) fn dialog_open(&self) -> bool {
        self.confirm_delete.is_some()
    }
}

impl App {
    pub(super) fn transactions(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Transactions");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("📥 Import CSV…")
                    .on_hover_text("Add transactions from a CSV file, such as a bank statement. You can also drop the file on the window.")
                    .clicked()
                {
                    self.choose_import_file();
                }
            });
        });
        ui.add_space(8.0);
        self.transaction_form(ui);
        ui.add_space(12.0);
        ui.separator();
        self.transaction_filters(ui);
        ui.add_space(6.0);
        self.transaction_table(ui);
        self.confirm_delete_dialog(ui);
    }

    fn transaction_form(&mut self, ui: &mut egui::Ui) {
        // Keep the selected category valid as categories change.
        if self.tx.bucket.is_none_or(|b| !self.categories.iter().any(|c| c.id == b)) {
            self.tx.bucket = self.categories.first().map(|c| c.id);
        }
        // And the team, as teams change.
        if self.tx.team.is_some_and(|t| self.team_name(t).is_none()) {
            self.tx.team = None;
        }

        let title = match self.tx.editing {
            Some(line) => format!("Edit transaction #{line}"),
            None => "Add a transaction".into(),
        };
        egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
            ui.label(RichText::new(title).strong());
            ui.add_space(4.0);
            let mut submit = false;
            egui::Grid::new("tx_form").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                ui.label("Description");
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.tx.name)
                        .hint_text("e.g. Weekly shop")
                        .char_limit(160)
                        .desired_width(320.0),
                );
                if std::mem::take(&mut self.tx.focus_name) {
                    r.request_focus();
                }
                submit |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.end_row();

                ui.label("Date");
                ui.add(
                    DatePickerButton::new(&mut self.tx.date)
                        .id_salt("tx_date")
                        .calendar_week(false)
                        .format(self.locale.date_pattern()),
                );
                ui.end_row();

                ui.label("Amount");
                ui.horizontal(|ui| {
                    if !self.locale.symbol_after {
                        ui.label(&self.locale.currency_symbol);
                    }
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.tx.amount)
                            .hint_text(self.locale.format_amount(0))
                            .desired_width(100.0),
                    );
                    submit |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if self.locale.symbol_after {
                        ui.label(&self.locale.currency_symbol);
                    }
                    ui.label(RichText::new("negative for refunds").small().weak());
                });
                ui.end_row();

                ui.label("Category");
                let selected = self
                    .categories
                    .iter()
                    .find(|c| Some(c.id) == self.tx.bucket)
                    .map_or("Choose…", |c| c.name.as_str());
                egui::ComboBox::from_id_salt("tx_category").selected_text(selected).width(200.0).show_ui(ui, |ui| {
                    for c in &self.categories {
                        ui.selectable_value(&mut self.tx.bucket, Some(c.id), &c.name);
                    }
                });
                ui.end_row();

                // Only on a shared database, for someone in a team.
                if !self.teams.is_empty() {
                    ui.label("Share with");
                    let selected = self.tx.team.and_then(|t| self.team_name(t)).unwrap_or("Only me");
                    egui::ComboBox::from_id_salt("tx_team")
                        .selected_text(selected)
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.tx.team, None, "Only me");
                            for t in &self.teams {
                                ui.selectable_value(&mut self.tx.team, Some(t.id), &t.name);
                            }
                        })
                        .response
                        .on_hover_text("Everyone in the team can see a shared transaction. Only you can change it.");
                    ui.end_row();
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let label = if self.tx.editing.is_some() { "💾 Save changes" } else { "➕ Add" };
                let save = ui.ctx().format_shortcut(&super::shortcuts::SAVE);
                submit |= ui.add(egui::Button::new(label).shortcut_text(save)).clicked();
                if self.tx.editing.is_some()
                    && ui.add(egui::Button::new("Cancel").shortcut_text("Esc")).clicked()
                {
                    self.tx.clear_form();
                }
            });
            if submit {
                self.submit_transaction();
            }
        });
    }

    pub(super) fn submit_transaction(&mut self) {
        let pence = match money::parse(&self.tx.amount) {
            Ok(p) => p,
            Err(e) => return self.set_error(e),
        };
        let Some(bucket) = self.tx.bucket else {
            return self.set_error("Add a category first (Categories tab)".into());
        };
        let s = Spending {
            line: self.tx.editing.unwrap_or(0),
            name: self.tx.name.trim().to_string(),
            date: self.tx.date,
            pence,
            bucket,
            owner: None,
            team: self.tx.team,
        };
        let ok = match self.tx.editing {
            Some(line) => self.run(&format!("Updated transaction #{line}"), |db| db.update_spending(&s)),
            None => self.run(&format!("Added '{}' ({})", s.name, self.money(s.pence)), |db| db.add_spending(&s)),
        };
        if ok {
            // Ready for the next one.
            self.tx.start_new();
        }
    }

    fn transaction_filters(&mut self, ui: &mut egui::Ui) {
        let months: BTreeSet<(i16, i8)> =
            self.spending.iter().map(|s| month_key(s.date)).chain([month_key(today())]).collect();
        ui.horizontal_wrapped(|ui| {
            ui.label("Month");
            let text = self.tx.month.map_or("All time".to_string(), |m| self.locale.month_label(m));
            egui::ComboBox::from_id_salt("filter_month").selected_text(text).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.tx.month, None, "All time");
                for m in months.iter().rev() {
                    ui.selectable_value(&mut self.tx.month, Some(*m), self.locale.month_label(*m));
                }
            });
            ui.add_space(8.0);
            ui.label("Category");
            let text = self
                .tx
                .category
                .and_then(|id| self.categories.iter().find(|c| c.id == id))
                .map_or("All categories", |c| c.name.as_str());
            egui::ComboBox::from_id_salt("filter_category").selected_text(text).show_ui(ui, |ui| {
                ui.selectable_value(&mut self.tx.category, None, "All categories");
                for c in &self.categories {
                    ui.selectable_value(&mut self.tx.category, Some(c.id), &c.name);
                }
            });
            ui.add_space(8.0);
            let hint = format!("🔍 Search ({})", ui.ctx().format_shortcut(&super::shortcuts::SEARCH));
            let r = ui.add(egui::TextEdit::singleline(&mut self.tx.search).hint_text(hint).desired_width(160.0));
            if std::mem::take(&mut self.tx.focus_search) {
                r.request_focus();
            }
            if ui.button("Reset").clicked() {
                self.tx.month = None;
                self.tx.category = None;
                self.tx.search.clear();
            }
        });
    }

    fn transaction_table(&mut self, ui: &mut egui::Ui) {
        let search = self.tx.search.to_lowercase();
        let rows: Vec<Spending> = self
            .spending
            .iter()
            .filter(|s| self.tx.month.is_none_or(|m| month_key(s.date) == m))
            .filter(|s| self.tx.category.is_none_or(|c| s.bucket == c))
            .filter(|s| search.is_empty() || s.name.to_lowercase().contains(&search))
            .cloned()
            .collect();
        let total: i64 = rows.iter().map(|s| s.pence).sum();
        ui.label(RichText::new(format!("{} transaction(s) · total {}", rows.len(), self.money(total))).strong());
        ui.add_space(4.0);

        if rows.is_empty() {
            ui.label(RichText::new("No transactions match.").weak());
            return;
        }

        let names = self.category_names();
        let mut edit: Option<Spending> = None;
        let mut delete: Option<Spending> = None;
        TableBuilder::new(ui)
            .id_salt("tx_table")
            .striped(true)
            .vscroll(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(95.0))
            .column(Column::remainder().at_least(160.0).clip(true))
            .column(Column::initial(150.0).clip(true))
            .column(Column::initial(100.0))
            .column(Column::exact(80.0))
            .header(22.0, |mut h| {
                for title in ["Date", "Description", "Category", "Amount", ""] {
                    h.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|mut body| {
                for s in &rows {
                    body.row(26.0, |mut row| {
                        row.col(|ui| {
                            ui.label(self.locale.format_date(s.date));
                        });
                        row.col(|ui| {
                            ui.label(&s.name);
                            if let Some(team) = s.team.and_then(|t| self.team_name(t)) {
                                let note = if self.can_change(s) { "Shared with" } else { "Shared by a member of" };
                                ui.label(RichText::new("👥").weak()).on_hover_text(format!("{note} {team}"));
                            }
                        });
                        row.col(|ui| {
                            ui.label(names.get(&s.bucket).copied().unwrap_or("(unknown)"));
                        });
                        row.col(|ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(self.locale.format_money(s.pence));
                            });
                        });
                        row.col(|ui| {
                            if !self.can_change(s) {
                                ui.label(RichText::new("🔒").weak())
                                    .on_hover_text("Shared by someone else in your team; only they can change it");
                                return;
                            }
                            if ui.small_button("✏").on_hover_text("Edit").clicked() {
                                edit = Some(s.clone());
                            }
                            if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                                delete = Some(s.clone());
                            }
                        });
                    });
                }
            });

        if let Some(s) = edit {
            self.tx.focus_name = true;
            self.tx.editing = Some(s.line);
            self.tx.name = s.name;
            self.tx.date = s.date;
            self.tx.amount = self.locale.format_amount(s.pence);
            self.tx.bucket = Some(s.bucket);
            self.tx.team = s.team;
        }
        if delete.is_some() {
            self.tx.confirm_delete = delete;
        }
    }

    fn confirm_delete_dialog(&mut self, ui: &mut egui::Ui) {
        let Some(s) = self.tx.confirm_delete.clone() else { return };
        let mut close = false;
        let mut confirmed = false;
        let response = egui::Modal::new(egui::Id::unique("confirm_delete_tx")).show(ui.ctx(), |ui| {
            ui.heading("Delete transaction?");
            ui.add_space(6.0);
            ui.label(format!("{} — {} on {}", s.name, self.money(s.pence), self.locale.format_date(s.date)));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("🗑 Delete").clicked() {
                    confirmed = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if confirmed {
            self.run(&format!("Deleted '{}'", s.name), |db| db.delete_spending(s.line));
            if self.tx.editing == Some(s.line) {
                self.tx.clear_form();
            }
        }
        if confirmed || close || response.should_close() {
            self.tx.confirm_delete = None;
        }
    }
}
