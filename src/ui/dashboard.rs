use std::collections::HashMap;

use egui::RichText;
use egui_plot::{Bar, BarChart, Plot};

use super::{App, today};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GroupBy {
    #[default]
    Month,
    Category,
}

impl GroupBy {
    fn label(self) -> &'static str {
        match self {
            GroupBy::Month => "Month",
            GroupBy::Category => "Category",
        }
    }
}

#[derive(Default)]
pub struct State {
    group_by: GroupBy,
}

impl App {
    pub(super) fn dashboard(&mut self, ui: &mut egui::Ui) {
        let year = today().year();
        let total: i64 = self.spending.iter().filter(|s| s.date.year() == year).map(|s| s.pence).sum();

        ui.horizontal(|ui| {
            ui.heading("Dashboard");
            ui.add_space(16.0);
            ui.label("Group by");
            egui::ComboBox::from_id_salt("dashboard_group_by").selected_text(self.dash.group_by.label()).show_ui(
                ui,
                |ui| {
                    for g in [GroupBy::Month, GroupBy::Category] {
                        ui.selectable_value(&mut self.dash.group_by, g, g.label());
                    }
                },
            );
        });
        ui.label(RichText::new(format!("Total spend in {year}: {}", self.money(total))).strong());
        ui.add_space(8.0);

        // (x label, pence, colour) per bar.
        let in_year = self.spending.iter().filter(|s| s.date.year() == year);
        let bars: Vec<(String, i64, egui::Color32)> = match self.dash.group_by {
            GroupBy::Month => {
                let mut months = [0i64; 12];
                for s in in_year {
                    months[(s.date.month() - 1) as usize] += s.pence;
                }
                let colour = ui.visuals().selection.bg_fill;
                months.iter().enumerate().map(|(i, p)| (self.locale.month_abbr3(i as i8 + 1), *p, colour)).collect()
            }
            GroupBy::Category => {
                let mut by_cat: HashMap<i64, i64> = HashMap::new();
                for s in in_year {
                    *by_cat.entry(s.bucket).or_default() += s.pence;
                }
                let mut rows: Vec<_> = self
                    .categories
                    .iter()
                    .map(|c| (c.name.clone(), by_cat.get(&c.id).copied().unwrap_or(0), self.category_colour(c.id)))
                    .collect();
                rows.sort_by_key(|r| std::cmp::Reverse(r.1));
                rows
            }
        };

        if total == 0 {
            ui.label(RichText::new(format!("No spending recorded in {year} yet.")).weak());
        }

        let labels: Vec<String> = bars.iter().map(|b| wrap_label(&b.0)).collect();
        let loc = self.locale.clone();
        let chart = BarChart::new(
            "Spend",
            bars.iter()
                .enumerate()
                .map(|(i, (name, pence, colour))| {
                    Bar::new(i as f64, *pence as f64 / 100.0).width(0.7).name(name).fill(*colour)
                })
                .collect(),
        )
        .element_formatter(Box::new(move |bar, _| {
            format!("{}\n{}", bar.name, loc.format_money((bar.value * 100.0).round() as i64))
        }));

        let height = (ui.available_height() - 8.0).max(280.0);
        let symbol = self.locale.currency_symbol.clone();
        Plot::new(("dashboard_chart", self.dash.group_by as u8))
            .height(height)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .include_y(0.0)
            .include_x(-0.6)
            .include_x(labels.len() as f64 - 0.4)
            .show_x(false)
            .y_axis_label(format!("Spend ({symbol})"))
            .x_axis_formatter(move |mark, _| {
                let v = mark.value;
                if v.fract() == 0.0 && v >= 0.0 {
                    labels.get(v as usize).cloned().unwrap_or_default()
                } else {
                    String::new()
                }
            })
            .show(ui, |plot| plot.bar_chart(chart));
    }
}

/// Split long axis labels over two lines at the space nearest the middle,
/// so the plot doesn't hide them for being wider than a bar slot.
fn wrap_label(name: &str) -> String {
    if name.chars().count() <= 12 {
        return name.to_string();
    }
    let mid = name.len() / 2;
    match name.match_indices(' ').map(|(i, _)| i).min_by_key(|i| i.abs_diff(mid)) {
        Some(i) => format!("{}\n{}", &name[..i], &name[i + 1..]),
        None => name.to_string(),
    }
}
