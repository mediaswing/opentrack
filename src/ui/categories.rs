use std::collections::HashMap;

use egui::RichText;

use super::App;

#[derive(Default)]
pub struct State {
    new_name: String,
    /// `(id, draft name)` while renaming.
    renaming: Option<(i64, String)>,
}

impl State {
    pub(super) fn cancel_rename(&mut self) {
        self.renaming = None;
    }
}

impl App {
    pub(super) fn categories(&mut self, ui: &mut egui::Ui) {
        ui.heading("Categories");
        ui.label(RichText::new("Group your spending. A category can only be deleted once nothing uses it.").weak());
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.cats.new_name)
                    .hint_text("New category name")
                    .char_limit(32)
                    .desired_width(240.0),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("➕ Add category").clicked() || enter {
                let name = self.cats.new_name.trim().to_string();
                if self.run(&format!("Added category '{name}'"), |db| db.add_category(&name)) {
                    self.cats.new_name.clear();
                }
            }
        });
        ui.add_space(10.0);

        let mut usage: HashMap<i64, (usize, i64)> = HashMap::new();
        for s in &self.spending {
            let e = usage.entry(s.bucket).or_default();
            e.0 += 1;
            e.1 += s.pence;
        }

        enum Action {
            StartRename(i64, String),
            SaveRename(i64, String),
            CancelRename,
            Delete(i64, String),
        }
        let mut action = None;

        let rows: Vec<(i64, String, usize, String, egui::Color32)> = self
            .categories
            .iter()
            .map(|c| {
                let (count, total) = usage.get(&c.id).copied().unwrap_or_default();
                (c.id, c.name.clone(), count, self.money(total), self.category_colour(c.id))
            })
            .collect();
        let renaming = &mut self.cats.renaming;

        egui::Grid::new("categories").striped(true).num_columns(4).spacing([16.0, 8.0]).show(ui, |ui| {
            ui.strong("Name");
            ui.strong("Transactions");
            ui.strong("Total spent");
            ui.label("");
            ui.end_row();

            for (cid, name, count, total, colour) in &rows {
                let (cid, count) = (*cid, *count);
                match renaming.as_mut() {
                    Some((id, draft)) if *id == cid => {
                        let r = ui.add(egui::TextEdit::singleline(draft).char_limit(32).desired_width(200.0));
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            action = Some(Action::SaveRename(cid, draft.clone()));
                        }
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 5.0, *colour);
                            ui.label(name);
                        });
                    }
                }
                ui.label(count.to_string());
                ui.label(total);
                ui.horizontal(|ui| {
                    if let Some((id, draft)) = renaming.as_ref()
                        && *id == cid
                    {
                        if ui.small_button("💾 Save").clicked() {
                            action = Some(Action::SaveRename(cid, draft.clone()));
                        }
                        if ui.small_button("Cancel").on_hover_text("Esc").clicked() {
                            action = Some(Action::CancelRename);
                        }
                    } else {
                        if ui.small_button("✏ Rename").clicked() {
                            action = Some(Action::StartRename(cid, name.clone()));
                        }
                        let delete = ui.add_enabled(count == 0, egui::Button::new("🗑 Delete").small());
                        let delete =
                            if count > 0 { delete.on_disabled_hover_text("In use by transactions") } else { delete };
                        if delete.clicked() {
                            action = Some(Action::Delete(cid, name.clone()));
                        }
                    }
                });
                ui.end_row();
            }
        });

        match action {
            Some(Action::StartRename(id, name)) => self.cats.renaming = Some((id, name)),
            Some(Action::CancelRename) => self.cats.renaming = None,
            Some(Action::SaveRename(id, name)) => {
                if self.run(&format!("Renamed category to '{}'", name.trim()), |db| db.rename_category(id, &name)) {
                    self.cats.renaming = None;
                }
            }
            Some(Action::Delete(id, name)) => {
                self.run(&format!("Deleted category '{name}'"), |db| db.delete_category(id));
            }
            None => {}
        }
    }
}
