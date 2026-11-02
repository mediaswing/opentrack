use std::collections::{BTreeSet, HashMap};

use egui::RichText;
use egui_plot::{Bar, BarChart, Legend, Plot};

use super::{App, today};

#[derive(Default)]
pub struct State {
    year: Option<i16>,
}

impl App {
    pub(super) fn reports(&mut self, ui: &mut egui::Ui) {
        let current_year = today().year();
        let years: BTreeSet<i16> = self.spending.iter().map(|s| s.date.year()).chain([current_year]).collect();
        let year = *self.reports.year.get_or_insert(current_year);

        ui.horizontal(|ui| {
            ui.heading("Reports");
            ui.add_space(16.0);
            ui.label("Year");
            egui::ComboBox::from_id_salt("report_year").selected_text(year.to_string()).show_ui(ui, |ui| {
                for y in years.iter().rev() {
                    ui.selectable_value(&mut self.reports.year, Some(*y), y.to_string());
                }
            });
        });
        ui.add_space(8.0);

        // totals[category][month 0..12]
        let mut totals: HashMap<i64, [i64; 12]> = HashMap::new();
        let mut month_totals = [0i64; 12];
        for s in self.spending.iter().filter(|s| s.date.year() == year) {
            let m = (s.date.month() - 1) as usize;
            totals.entry(s.bucket).or_insert([0; 12])[m] += s.pence;
            month_totals[m] += s.pence;
        }
        let year_total: i64 = month_totals.iter().sum();

        if year_total == 0 && totals.is_empty() {
            ui.label(RichText::new(format!("No spending recorded in {year}.")).weak());
            return;
        }

        // Category order: biggest spend first.
        let names = self.category_names();
        let mut order: Vec<(i64, i64)> = totals.iter().map(|(id, m)| (*id, m.iter().sum())).collect();
        order.sort_by_key(|r| std::cmp::Reverse(r.1));

        // Months with data so far, for the average.
        let active_months = if year == current_year { today().month() as i64 } else { 12 };

        ui.horizontal_wrapped(|ui| {
            super::card(ui, &format!("Total {year}"), &self.money(year_total), "");
            super::card(ui, "Monthly average", &self.money(year_total / active_months.max(1)), "");
            // The earliest month wins a tie. No month with spending, no card.
            if let Some((m, v)) = highest_month(&month_totals) {
                super::card(ui, "Highest month", &self.money(v), self.locale.month_short(m as i8 + 1));
            }
        });
        ui.add_space(10.0);

        // Stacked bar chart: one series per category.
        let symbol = self.locale.currency_symbol.clone();
        let mut charts: Vec<BarChart> = Vec::new();
        for (id, _) in &order {
            let name = names.get(id).copied().unwrap_or("(unknown)").to_string();
            let bars: Vec<Bar> = totals[id]
                .iter()
                .enumerate()
                .map(|(m, p)| {
                    Bar::new((m + 1) as f64, *p as f64 / 100.0).width(0.7).name(self.locale.month_short(m as i8 + 1))
                })
                .collect();
            let loc = self.locale.clone();
            let chart = BarChart::new(name.clone(), bars).color(self.category_colour(*id)).element_formatter(Box::new(
                move |bar, _| format!("{name}\n{}: {}", bar.name, loc.format_money((bar.value * 100.0).round() as i64)),
            ));
            let below: Vec<&BarChart> = charts.iter().collect();
            let chart = chart.stack_on(&below);
            charts.push(chart);
        }
        Plot::new("monthly_spend")
            .height(320.0)
            .legend(Legend::default())
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .include_y(0.0)
            .include_x(0.4)
            .include_x(12.6)
            .show_x(false)
            .y_axis_label(format!("Spending ({symbol})"))
            .x_axis_formatter(|mark, _| {
                let loc = &self.locale;
                let v = mark.value;
                if v.fract() == 0.0 && (1.0..=12.0).contains(&v) {
                    loc.month_short(v as i8).to_string()
                } else {
                    String::new()
                }
            })
            .show(ui, |plot| {
                for chart in charts {
                    plot.bar_chart(chart);
                }
            });

        ui.add_space(12.0);
        ui.label(RichText::new(format!("Category breakdown for {year}")).strong());
        ui.add_space(4.0);
        egui::Grid::new("report_table").striped(true).num_columns(4).spacing([20.0, 6.0]).show(ui, |ui| {
            ui.strong("Category");
            ui.strong("Total");
            ui.strong("Share");
            ui.strong("Monthly avg");
            ui.end_row();
            for (id, total) in &order {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 5.0, self.category_colour(*id));
                    ui.label(names.get(id).copied().unwrap_or("(unknown)"));
                });
                ui.label(self.money(*total));
                let share = if year_total != 0 { *total as f64 / year_total as f64 * 100.0 } else { 0.0 };
                ui.label(format!("{share:.1}%"));
                ui.label(self.money(total / active_months.max(1)));
                ui.end_row();
            }
            ui.strong("Total");
            ui.strong(self.money(year_total));
            ui.label("100%");
            ui.label(self.money(year_total / active_months.max(1)));
            ui.end_row();
        });
    }
}

/// The month (0-based) with the most spending and its total. The earliest
/// month wins a tie; `None` when no month has more than zero.
fn highest_month(month_totals: &[i64; 12]) -> Option<(usize, i64)> {
    month_totals.iter().copied().enumerate().filter(|(_, v)| *v > 0).fold(None, |best, (m, v)| match best {
        Some((_, b)) if b >= v => best,
        _ => Some((m, v)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highest_month_prefers_the_earliest_and_ignores_empty_years() {
        let mut totals = [0i64; 12];
        assert_eq!(highest_month(&totals), None);
        totals[11] = -500;
        assert_eq!(highest_month(&totals), None, "refunds alone are not a highest month");
        totals[2] = 900;
        totals[7] = 900;
        assert_eq!(highest_month(&totals), Some((2, 900)));
        totals[9] = 901;
        assert_eq!(highest_month(&totals), Some((9, 901)));
    }
}
