//! The "Import CSV" dialog: pick a file (or drop one on the window), check
//! which column is which, preview, and import.

use std::path::Path;

use egui::RichText;

use super::App;
use crate::import::{self, Mapping, Options, Plan, Table};
use crate::locale::DateOrder;

/// Rows shown in the preview.
const PREVIEW_ROWS: usize = 8;
/// Unreadable lines listed by number.
const ERRORS_LISTED: usize = 5;

#[derive(Default)]
pub struct State {
    dialog: Option<Dialog>,
}

impl State {
    pub(super) fn is_open(&self) -> bool {
        self.dialog.is_some()
    }
}

struct Dialog {
    file_name: String,
    table: Table,
    mapping: Mapping,
    options: Options,
    skip_errors: bool,
    /// The plan for `planned_for`, worked out again only when the settings change.
    plan: Plan,
    planned_for: Option<(Mapping, Options)>,
}

impl App {
    /// Ask for a CSV file with the system's file dialog.
    pub(super) fn choose_import_file(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("Import transactions from CSV")
            .add_filter("CSV files", &["csv", "txt", "tsv"])
            .add_filter("All files", &["*"])
            .pick_file();
        if let Some(path) = picked {
            self.open_import(&path);
        }
    }

    /// Read `path` and show the import dialog for it.
    pub(super) fn open_import(&mut self, path: &Path) {
        if self.store.is_none() {
            return self.set_error("No database is connected — check Settings".into());
        }
        log::info!("Reading {} for import", path.display());
        let table = match Table::read(path) {
            Ok(t) => t,
            Err(e) => {
                log::error!("Import failed: {e:#}");
                return self.set_error(format!("Could not import: {e:#}"));
            }
        };
        let mapping = Mapping::guess(&table, self.locale.date_order);
        log::debug!("Import columns guessed as {mapping:?}");
        // Unknown categories go into the most used one, unless they are created.
        let mut uses = std::collections::HashMap::<i64, usize>::new();
        for s in &self.spending {
            *uses.entry(s.bucket).or_default() += 1;
        }
        let default_category =
            self.categories.iter().max_by_key(|c| (uses.get(&c.id), std::cmp::Reverse(&c.name))).map(|c| c.id);
        self.import.dialog = Some(Dialog {
            file_name: path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into()),
            table,
            mapping,
            options: Options { default_category, create_categories: true, skip_duplicates: true },
            skip_errors: false,
            plan: Plan::default(),
            planned_for: None,
        });
    }

    pub(super) fn import_dialog(&mut self, ctx: &egui::Context) {
        let Some(d) = self.import.dialog.as_mut() else { return };
        let key = (d.mapping.clone(), d.options.clone());
        if d.planned_for.as_ref() != Some(&key) {
            let plan = import::plan(d.mapping.rows(&d.table), &self.categories, &self.spending, &d.options);
            // Agreeing to leave out some lines doesn't agree to leave out others.
            if plan.errors != d.plan.errors {
                d.skip_errors = false;
            }
            d.plan = plan;
            d.planned_for = Some(key);
        }

        let mut close = false;
        let mut confirmed = false;
        let width = (ctx.content_rect().width() - 80.0).clamp(320.0, 680.0);
        let response = egui::Modal::new(egui::Id::unique("import_csv")).show(ctx, |ui| {
            ui.set_width(width);
            ui.heading("Import transactions");
            ui.add_space(4.0);
            let lines = d.mapping.data(&d.table).count();
            ui.label(format!("{} — {lines} line(s), separated by {}", d.file_name, d.table.delimiter_name()));
            if d.table.legacy_encoding {
                ui.label(RichText::new("The file isn't UTF-8, so it was read as Windows-1252.").small().weak());
            }
            ui.add_space(8.0);
            mapping_form(ui, d, &self.categories);
            ui.add_space(8.0);
            ui.separator();
            summary(ui, d, &self.locale);
            ui.add_space(8.0);

            let n = d.plan.rows.len();
            let ready = n > 0 && (d.plan.errors.is_empty() || d.skip_errors);
            ui.horizontal(|ui| {
                let label = format!("📥 Import {n} transaction{}", if n == 1 { "" } else { "s" });
                if ui.add_enabled(ready, egui::Button::new(label)).clicked() {
                    confirmed = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });

        if confirmed {
            let rows = std::mem::take(&mut d.plan.rows);
            let new = d.plan.new_categories.len();
            let mut what = format!("Imported {} transaction(s) from {}", rows.len(), d.file_name);
            if new > 0 {
                what.push_str(&format!(", adding {new} categor{}", if new == 1 { "y" } else { "ies" }));
            }
            if self.run(&what, |db| db.import(&rows)) {
                self.import.dialog = None;
                // Imports are usually from earlier months; show them.
                self.tx.show_all_months();
            } else {
                // Work the plan out again; the import may have failed part way.
                if let Some(d) = self.import.dialog.as_mut() {
                    d.planned_for = None;
                }
            }
        } else if close || response.should_close() {
            self.import.dialog = None;
        }
    }
}

/// Column name for the combo boxes, with an example cell.
fn column_label(table: &Table, mapping: &Mapping, col: usize) -> String {
    let name = if mapping.has_header {
        table.records[0].1.get(col).filter(|h| !h.is_empty()).cloned().unwrap_or_else(|| format!("Column {}", col + 1))
    } else {
        format!("Column {}", col + 1)
    };
    let example = mapping.data(table).find_map(|(_, r)| r.get(col).filter(|c| !c.is_empty()));
    match example {
        Some(e) => {
            let short: String = e.chars().take(24).collect();
            let more = if e.chars().count() > 24 { "…" } else { "" };
            format!("{name}  (e.g. {short}{more})")
        }
        None => name,
    }
}

fn column_combo(ui: &mut egui::Ui, id: &str, table: &Table, mapping: &mut Mapping, pick: fn(&mut Mapping) -> &mut Option<usize>, optional: bool) {
    let labels: Vec<String> = (0..table.columns()).map(|c| column_label(table, mapping, c)).collect();
    let current = *pick(mapping);
    let text = current.and_then(|c| labels.get(c).cloned()).unwrap_or_else(|| {
        if optional { "None".into() } else { "Choose…".into() }
    });
    egui::ComboBox::from_id_salt(id).selected_text(text).width(320.0).show_ui(ui, |ui| {
        let slot = pick(mapping);
        if optional {
            ui.selectable_value(slot, None, "None");
        }
        for (c, label) in labels.iter().enumerate() {
            ui.selectable_value(slot, Some(c), label);
        }
    });
}

fn mapping_form(ui: &mut egui::Ui, d: &mut Dialog, categories: &[crate::db::Category]) {
    let Dialog { table, mapping, options, .. } = d;
    egui::Grid::new("import_mapping").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("");
        ui.checkbox(&mut mapping.has_header, "The first line names the columns");
        ui.end_row();

        ui.label("Date");
        ui.horizontal(|ui| {
            column_combo(ui, "import_date", table, mapping, |m| &mut m.date, false);
            let order = |o: DateOrder| match o {
                DateOrder::Dmy => "Day first",
                DateOrder::Mdy => "Month first",
                DateOrder::Ymd => "Year first",
            };
            egui::ComboBox::from_id_salt("import_date_order").selected_text(order(mapping.date_order)).show_ui(
                ui,
                |ui| {
                    for o in [DateOrder::Dmy, DateOrder::Mdy, DateOrder::Ymd] {
                        ui.selectable_value(&mut mapping.date_order, o, order(o));
                    }
                },
            )
            .response
            .on_hover_text("How to read dates like 02/11/2026. Dates that start with a four-digit year always work.");
        });
        ui.end_row();

        ui.label("Description");
        column_combo(ui, "import_name", table, mapping, |m| &mut m.name, false);
        ui.end_row();

        ui.label("Amount");
        ui.vertical(|ui| {
            column_combo(ui, "import_amount", table, mapping, |m| &mut m.amount, false);
            ui.checkbox(&mut mapping.negate, "Spending is negative in this file (as on most bank statements)");
        });
        ui.end_row();

        ui.label("Category");
        ui.vertical(|ui| {
            column_combo(ui, "import_category", table, mapping, |m| &mut m.category, true);
            ui.add_enabled(
                mapping.category.is_some(),
                egui::Checkbox::new(&mut options.create_categories, "Add categories that don't exist yet"),
            );
        });
        ui.end_row();

        let otherwise = if mapping.category.is_some() && options.create_categories {
            "Without a category"
        } else if mapping.category.is_some() {
            "Otherwise"
        } else {
            "Put them in"
        };
        ui.label(otherwise);
        let text = options
            .default_category
            .and_then(|id| categories.iter().find(|c| c.id == id))
            .map_or("Choose…", |c| c.name.as_str());
        egui::ComboBox::from_id_salt("import_default_category").selected_text(text).width(200.0).show_ui(ui, |ui| {
            for c in categories {
                ui.selectable_value(&mut options.default_category, Some(c.id), &c.name);
            }
        });
        ui.end_row();

        ui.label("");
        ui.checkbox(&mut options.skip_duplicates, "Leave out transactions that are already recorded")
            .on_hover_text("Same date, description and amount as one already in the database");
        ui.end_row();
    });
}

fn summary(ui: &mut egui::Ui, d: &mut Dialog, locale: &crate::locale::Locale) {
    let plan = &d.plan;
    let total: i64 = plan.rows.iter().map(|r| r.pence).sum();
    ui.label(RichText::new(format!("{} to import · total {}", plan.rows.len(), locale.format_money(total))).strong());
    if plan.duplicates > 0 {
        ui.label(format!("{} already recorded, left out", plan.duplicates));
    }
    if !plan.new_categories.is_empty() {
        ui.label(format!("New categories: {}", plan.new_categories.join(", ")));
    }
    if !plan.errors.is_empty() {
        let colour = ui.visuals().error_fg_color;
        let n = plan.errors.len();
        ui.label(RichText::new(format!("{n} line(s) can't be read:")).color(colour));
        for e in plan.errors.iter().take(ERRORS_LISTED) {
            ui.label(RichText::new(format!("  Line {}: {}", e.line, e.message)).color(colour).small());
        }
        if n > ERRORS_LISTED {
            ui.label(RichText::new(format!("  …and {} more", n - ERRORS_LISTED)).color(colour).small());
        }
        ui.checkbox(&mut d.skip_errors, format!("Import the rest and leave out these {n} line(s)"));
    }

    if plan.rows.is_empty() {
        return;
    }
    ui.add_space(6.0);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        egui::Grid::new("import_preview").striped(true).num_columns(4).spacing([12.0, 4.0]).show(ui, |ui| {
            for title in ["Date", "Description", "Category"] {
                ui.strong(title);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.strong("Amount");
            });
            ui.end_row();
            for r in plan.rows.iter().take(PREVIEW_ROWS) {
                ui.label(locale.format_date(r.date));
                let name: String = r.name.chars().take(40).collect();
                ui.label(name).on_hover_text(&r.name);
                ui.label(&r.category);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(locale.format_money(r.pence));
                });
                ui.end_row();
            }
        });
        if plan.rows.len() > PREVIEW_ROWS {
            ui.label(RichText::new(format!("…and {} more", plan.rows.len() - PREVIEW_ROWS)).small().weak());
        }
    });
}
