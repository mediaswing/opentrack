//! The Summary tab: the spending for the dates shown against the budget, as
//! bars for each category, and a chart of the months: the last twelve when
//! showing a month, or each month in a run of days.

use super::*;

/// How many months the chart shows when showing a month, ending with it.
const CHART_MONTHS: i32 = 12;

#[derive(Default)]
pub(super) struct Summary {
    /// Each category's spending for the dates shown, largest first.
    rows: Vec<Row>,
    pub(super) spent: i64,
    /// How many entries are on the dates shown.
    pub(super) count: i64,
    /// The spending in categories that have a budget, to compare with
    /// `budget`.
    budgeted_spent: i64,
    /// The total of the budgets that are set, for the dates shown.
    pub(super) budget: i64,
    /// Whether any category has a budget.
    has_budget: bool,
    months: Vec<(Month, i64)>,
}

struct Row {
    category: i64,
    spent: i64,
    /// The category's budget for the dates shown.
    budget: Option<i64>,
}

impl BudgetApp {
    pub(super) fn reload_summary(&mut self) {
        let period = self.period;
        let totals = match self.db.totals(period) {
            Ok(totals) => totals,
            Err(e) => return self.show_error(t!("error-sum-spending", error = format!("{e:#}"))),
        };
        let mut rows: Vec<Row> = self
            .categories
            .iter()
            .map(|c| Row {
                category: c.id,
                spent: totals.iter().find(|(id, _)| *id == c.id).map_or(0, |(_, t)| *t),
                budget: c.budget.map(|monthly| period.budget(monthly)),
            })
            .collect();
        // Largest first; the categories are already in alphabetical order.
        rows.sort_by_key(|r| std::cmp::Reverse(r.spent));
        let chart = match period {
            Period::Month(m) => Period::Days { first: m.plus(1 - CHART_MONTHS).first_day(), last: m.last_day() },
            days => days,
        };
        let months = match self.db.monthly_totals(chart) {
            Ok(months) => months,
            Err(e) => return self.show_error(t!("error-sum-spending", error = format!("{e:#}"))),
        };
        let count = match self.db.period_entry_count(period) {
            Ok(count) => count,
            Err(e) => return self.show_error(t!("error-count-entries", error = format!("{e:#}"))),
        };
        self.summary = Summary {
            count,
            spent: rows.iter().map(|r| r.spent).sum(),
            budgeted_spent: rows.iter().filter(|r| r.budget.is_some()).map(|r| r.spent).sum(),
            budget: rows.iter().filter_map(|r| r.budget).sum(),
            has_budget: rows.iter().any(|r| r.budget.is_some()),
            rows,
            months,
        };
    }

    /// The spending for the dates shown in a sentence or two, for F7 and
    /// the tab.
    pub(super) fn summary_text(&self) -> String {
        let s = &self.summary;
        let when = self.region.period_opening(self.period);
        let spent = self.money(s.spent);
        let budget = self.money(s.budget);
        // Spending in categories without a budget is in the total but not
        // compared with the budget.
        let budgeted = s.budgeted_spent;
        let left = self.money(s.budget - budgeted);
        let over = self.money(budgeted - s.budget);
        let mut text = if !s.has_budget {
            t!("summary-no-budgets", when = &when, spent = &spent)
        } else if budgeted == s.spent && budgeted <= s.budget {
            t!("summary-within", when = &when, spent = &spent, budget = &budget, left = left)
        } else if budgeted == s.spent {
            t!("summary-over", when = &when, spent = &spent, budget = &budget, over = over)
        } else if budgeted <= s.budget {
            t!("summary-part-within", when = &when, spent = &spent, budgeted = self.money(budgeted), budget = &budget, left = left)
        } else {
            t!("summary-part-over", when = &when, spent = &spent, budgeted = self.money(budgeted), budget = &budget, over = over)
        };
        let over: Vec<String> = s
            .rows
            .iter()
            .filter(|r| r.budget.is_some_and(|b| r.spent > b))
            .map(|r| self.category_name(r.category))
            .collect();
        let over = match over.as_slice() {
            [] => None,
            [one] => Some(t!("summary-one-over", name = one)),
            many => Some(t!("summary-many-over", count = many.len(), names = many.join(", "))),
        };
        if let Some(over) = over {
            text.push(' ');
            text.push_str(&over);
        }
        text
    }

    pub(super) fn summary_tab(&mut self, ui: &mut Ui) {
        self.period_controls(ui);
        let label = self.region.period_label(self.period);
        heading(ui, &t!("summary-heading", period = &label));
        note(ui, &self.summary_text());
        let s = &self.summary;
        if s.has_budget && matches!(self.period, Period::Days { .. }) {
            note(ui, &t!("summary-budget-by-day"));
        }
        if s.has_budget {
            let (amounts, spoken) = self.bar_text(&t!("summary-total-name"), s.budgeted_spent, Some(s.budget));
            let fraction = if s.budget > 0 { s.budgeted_spent as f32 / s.budget as f32 } else { 1.0 };
            spending_bar(ui, &t!("summary-total-bar"), &amounts, fraction, s.budgeted_spent > s.budget, true, &spoken);
        }

        heading(ui, &t!("summary-by-category"));
        let largest = s.rows.iter().map(|r| r.spent).max().unwrap_or(0).max(1);
        let mut hidden = 0;
        for row in &s.rows {
            if row.spent == 0 && row.budget.is_none() {
                hidden += 1;
                continue;
            }
            let name = self.category_name(row.category);
            let (amounts, spoken) = self.bar_text(&name, row.spent, row.budget);
            let (fraction, over) = match row.budget {
                Some(0) => (if row.spent > 0 { 1.0 } else { 0.0 }, row.spent > 0),
                Some(budget) => (row.spent as f32 / budget as f32, row.spent > budget),
                None => (row.spent as f32 / largest as f32, false),
            };
            spending_bar(ui, &name, &amounts, fraction, over, row.budget.is_some(), &spoken);
        }
        if hidden > 0 {
            note(ui, &t!("summary-hidden", count = hidden));
        }

        let (chart_heading, highlight) = match self.period {
            Period::Month(m) => (t!("summary-chart-heading", count = CHART_MONTHS, month = m.label()), true),
            Period::Days { .. } => (t!("summary-chart-dates-heading", period = &label), false),
        };
        heading(ui, &chart_heading);
        let s = &self.summary;
        let bars: Vec<(String, i64)> = s.months.iter().map(|(m, total)| (m.short_label(), *total)).collect();
        let highest = s.months.iter().map(|(_, t)| *t).max().unwrap_or(0);
        let listed: Vec<String> = s.months.iter().map(|(m, t)| format!("{} {}", m.label(), self.money(*t))).collect();
        let description = t!("summary-chart-description", months = listed.join(", "));
        let top = t!("summary-highest", amount = self.money(highest));
        month_chart(ui, &bars, highlight.then(|| bars.len().saturating_sub(1)), &top, &description);
        let average = s.months.iter().map(|(_, t)| *t).sum::<i64>() / (s.months.len().max(1) as i64);
        note(ui, &t!("summary-average", amount = self.money(average)));
    }

    /// The amounts shown beside a bar, and what a screen reader says for it.
    fn bar_text(&self, name: &str, spent: i64, budget: Option<i64>) -> (String, String) {
        let spent_text = self.money(spent);
        match budget {
            None => (spent_text.clone(), t!("summary-bar-no-budget", name = name, spent = &spent_text)),
            Some(budget) => {
                let budget_text = self.money(budget);
                let amounts = t!("summary-bar-amounts", spent = &spent_text, budget = &budget_text);
                let left = if spent <= budget {
                    t!("summary-bar-left", amount = self.money(budget - spent))
                } else {
                    t!("summary-bar-over", amount = self.money(spent - budget))
                };
                let spoken = t!("summary-bar-spoken", name = name, spent = &spent_text, budget = &budget_text, left = left);
                (amounts, spoken)
            }
        }
    }
}
