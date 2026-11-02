//! The window: tabs down the left, the selected pane on the right.

mod categories;
mod dashboard;
mod reports;
mod settings;
mod shortcuts;
mod transactions;

use std::collections::HashMap;

use egui::{Color32, RichText};
use jiff::civil::Date;

use crate::config::Config;
use crate::db::{self, Category, Spending, Store};
use crate::locale::Locale;
use crate::update;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dashboard,
    Transactions,
    Categories,
    Reports,
    Settings,
}

impl Tab {
    const ALL: [Tab; 5] = [Tab::Dashboard, Tab::Transactions, Tab::Categories, Tab::Reports, Tab::Settings];

    fn label(self) -> &'static str {
        match self {
            Tab::Dashboard => "🏠  Dashboard",
            Tab::Transactions => "💳  Transactions",
            Tab::Categories => "🏷  Categories",
            Tab::Reports => "📊  Reports",
            Tab::Settings => "⚙  Settings",
        }
    }
}

struct Status {
    text: String,
    error: bool,
}

pub struct App {
    cfg: Config,
    locale: Locale,
    store: Option<Box<dyn Store>>,
    categories: Vec<Category>,
    spending: Vec<Spending>,
    tab: Tab,
    status: Option<Status>,
    dash: dashboard::State,
    tx: transactions::State,
    cats: categories::State,
    reports: reports::State,
    settings: settings::State,
    /// An update check that is still running.
    update_check: Option<update::Check>,
    /// The last check's result, shown in Settings.
    update_outcome: Option<update::Outcome>,
    /// Show the "new version" notice in the status bar.
    update_notice: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (cfg, config_warning) = Config::load();
        let locale = cfg.resolve_locale();
        log::info!(
            "System locale: {}; using {} (currency '{}', dates {})",
            Locale::system_tag(),
            locale.tag,
            locale.currency_symbol,
            locale.date_pattern()
        );
        let mut app = Self {
            settings: settings::State::new(&cfg),
            cfg,
            locale,
            store: None,
            categories: Vec::new(),
            spending: Vec::new(),
            tab: Tab::Dashboard,
            status: None,
            dash: dashboard::State::default(),
            tx: transactions::State::default(),
            cats: categories::State::default(),
            reports: reports::State::default(),
            update_check: None,
            update_outcome: None,
            update_notice: false,
        };
        if let Some(warning) = config_warning {
            app.set_error(warning);
        }
        if app.cfg.check_for_updates {
            app.start_update_check(&cc.egui_ctx);
        } else {
            log::info!("Update checks are turned off");
        }
        match db::open(&app.cfg) {
            Ok(store) => {
                app.store = Some(store);
                app.reload();
            }
            Err(e) => {
                log::error!("Could not open database: {e:#}");
                // Keep a config warning visible too.
                let earlier = app.status.take().map(|s| format!(" {}", s.text)).unwrap_or_default();
                app.set_error(format!("Could not open database: {e:#}.{earlier}"));
                app.tab = Tab::Settings;
            }
        }
        app
    }

    /// Re-read categories and spending from the database.
    fn reload(&mut self) {
        let Some(store) = self.store.as_mut() else { return };
        let result = store.categories().and_then(|c| Ok((c, store.spending()?)));
        match result {
            Ok((categories, spending)) => {
                log::debug!("Loaded {} categories and {} transactions", categories.len(), spending.len());
                self.categories = categories;
                self.spending = spending;
            }
            Err(e) => {
                log::error!("Reload failed: {e:#}");
                self.set_error(format!("Could not read the database: {e:#}"));
            }
        }
    }

    /// Run a write against the store, report the outcome and reload. Returns true on success.
    fn run(&mut self, what: &str, op: impl FnOnce(&mut dyn Store) -> anyhow::Result<()>) -> bool {
        let Some(store) = self.store.as_mut() else {
            self.set_error("No database is connected — check Settings".into());
            return false;
        };
        log::debug!("{what}");
        match op(store.as_mut()) {
            Ok(()) => {
                self.set_info(what.to_string());
                self.reload();
                true
            }
            Err(e) => {
                log::error!("{what} failed: {e:#}");
                self.set_error(format!("{e:#}"));
                false
            }
        }
    }

    fn start_update_check(&mut self, ctx: &egui::Context) {
        if self.update_check.is_none() {
            log::debug!("Checking for updates");
            self.update_check = Some(update::Check::start(ctx.clone()));
        }
    }

    /// Pick up the result of a finished update check.
    fn poll_update_check(&mut self) {
        let Some(outcome) = self.update_check.as_ref().and_then(update::Check::poll) else { return };
        self.update_check = None;
        self.update_notice = matches!(outcome, update::Outcome::Available(_));
        self.update_outcome = Some(outcome);
    }

    fn set_info(&mut self, text: String) {
        self.status = Some(Status { text, error: false });
    }

    fn set_error(&mut self, text: String) {
        self.status = Some(Status { text, error: true });
    }

    fn money(&self, pence: i64) -> String {
        self.locale.format_money(pence)
    }

    fn category_names(&self) -> HashMap<i64, &str> {
        self.categories.iter().map(|c| (c.id, c.name.as_str())).collect()
    }

    /// A colour per category, the same on every tab. It depends only on the
    /// category's id, so adding or deleting other categories never changes it.
    /// Stepping the hue by the golden ratio keeps consecutive ids far apart.
    fn category_colour(&self, id: i64) -> Color32 {
        category_colour(id)
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let backend = self.store.as_ref().map_or("Not connected".to_string(), |s| s.describe());
            ui.label(RichText::new(backend).weak());
            if let (true, Some(update::Outcome::Available(release))) = (self.update_notice, &self.update_outcome) {
                ui.separator();
                ui.hyperlink_to(format!("⬆ Version {} is available", release.version), &release.url)
                    .on_hover_text(&release.url);
                if ui.small_button("✖").on_hover_text("Dismiss").clicked() {
                    self.update_notice = false;
                }
            }
            if let Some(status) = &self.status {
                ui.separator();
                let colour = if status.error { ui.visuals().error_fg_color } else { ui.visuals().text_color() };
                ui.label(RichText::new(&status.text).color(colour));
                if ui.small_button("✖").on_hover_text("Dismiss").clicked() {
                    self.status = None;
                }
            }
        });
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        for tab in Tab::ALL {
            let button = egui::Button::selectable(self.tab == tab, RichText::new(tab.label()).size(15.0))
                .min_size(egui::vec2(ui.available_width(), 34.0));
            let shortcut = ui.ctx().format_shortcut(&shortcuts::tab(tab));
            if ui.add(button).on_hover_text(shortcut).clicked() {
                self.switch_tab(tab);
            }
        }
    }

    fn switch_tab(&mut self, tab: Tab) {
        if self.tab != tab {
            log::debug!("Switched to tab {tab:?}");
            self.tab = tab;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_update_check();
        self.handle_shortcuts(&ui.ctx().clone());
        egui::Panel::bottom("status_bar").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("tabs").resizable(false).exact_size(190.0).show(ui, |ui| self.tab_bar(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            if self.tab == Tab::Dashboard {
                // The chart fills the whole pane.
                self.dashboard(ui);
                return;
            }
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                // Keep forms and tables to a readable width instead of the full window.
                let width = ui.available_width().min(CONTENT_MAX_WIDTH);
                let layout = egui::Layout::top_down(egui::Align::Min);
                ui.allocate_ui_with_layout(egui::vec2(width, 0.0), layout, |ui| {
                    ui.set_width(width);
                    match self.tab {
                        Tab::Transactions => self.transactions(ui),
                        Tab::Categories => self.categories(ui),
                        Tab::Reports => self.reports(ui),
                        Tab::Settings => self.settings(ui),
                        Tab::Dashboard => {}
                    }
                });
            });
        });
    }
}

// ---- small shared helpers ----

/// Maximum width of the content on the non-dashboard tabs.
const CONTENT_MAX_WIDTH: f32 = 760.0;

fn category_colour(id: i64) -> Color32 {
    let hue = (id as f64 * 0.618_033_988_749_895).rem_euclid(1.0) as f32;
    egui::ecolor::Hsva::new(hue, 0.55, 0.85, 1.0).into()
}

pub(crate) fn today() -> Date {
    jiff::Zoned::now().date()
}

/// `(year, month)` key for grouping by month.
pub(crate) fn month_key(d: Date) -> (i16, i8) {
    (d.year(), d.month())
}

pub(crate) fn card(ui: &mut egui::Ui, title: &str, value: &str, sub: &str) {
    egui::Frame::group(ui.style()).inner_margin(12.0).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.set_min_width(170.0);
            ui.label(RichText::new(title).weak());
            ui.label(RichText::new(value).size(22.0).strong());
            ui.label(RichText::new(if sub.is_empty() { " " } else { sub }).small().weak());
        });
    });
}
