//! The window: tabs down the left, the selected pane on the right, and a status
//! bar along the bottom. It follows Common Speech Output's layout and rules.
//!
//! Accessibility comes first here:
//! * egui publishes the interface to screen readers through AccessKit (UI
//!   Automation on Windows, NSAccessibility on macOS).
//! * Every field and dropdown is linked to its visible label, tabs have the tab
//!   role and selected state, and the status bar is a live region, so what
//!   happened, and any mistake, is announced without moving the focus.
//! * Everything works from the keyboard: Tab and Shift+Tab move between
//!   controls, Space or Enter activates them, and the shortcuts in
//!   `shortcuts.rs` work from anywhere. Tests in `tests.rs` hold this in place.
//! * When a control the focus is on goes away, or a mistake needs fixing, the
//!   focus moves somewhere sensible, so nobody is left stranded.

mod add;
mod appearance;
mod categories;
mod import_export;
mod months;
mod settings;
mod shortcuts;
#[cfg(test)]
mod tests;
mod updates;
mod widgets;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use chrono::{Datelike, Local, NaiveDate};
use egui::accesskit::{Live, Role};
use egui::{RichText, Ui};

pub use appearance::Appearance;

use crate::db::{Category, CategoryTotal, Db, Spending};
use crate::locale::LocaleFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Summary,
    Spending,
    Categories,
    ImportExport,
    Settings,
}

impl Tab {
    const ALL: [Tab; 5] = [Tab::Summary, Tab::Spending, Tab::Categories, Tab::ImportExport, Tab::Settings];

    fn name(self) -> &'static str {
        match self {
            Tab::Summary => "Summary",
            Tab::Spending => "Spending",
            Tab::Categories => "Categories",
            Tab::ImportExport => "Import/Export",
            Tab::Settings => "System Settings",
        }
    }

    /// With an icon, as on the window. Screen readers get `name` instead.
    fn label(self) -> &'static str {
        match self {
            Tab::Summary => "📊  Summary",
            Tab::Spending => "➕  Spending",
            Tab::Categories => "🏷  Categories",
            Tab::ImportExport => "⬍  Import/Export",
            Tab::Settings => "⚙  System Settings",
        }
    }
}

/// The native dialogs, behind a trait so the tests can stand in for them.
pub trait Dialogs {
    fn confirm(&self, title: &str, question: &str) -> bool;
    /// Asks which file to open, of those whose names end in one of `extensions`.
    fn open_file(&self, title: &str, kind: &str, extensions: &[&str]) -> Option<PathBuf>;
    /// Asks where to save a file, suggesting `name`.
    fn save_file(&self, title: &str, name: &str, kind: &str, extensions: &[&str]) -> Option<PathBuf>;
}

/// The operating system's own dialogs, which screen readers already know.
pub struct NativeDialogs;

impl Dialogs for NativeDialogs {
    fn confirm(&self, title: &str, question: &str) -> bool {
        rfd::MessageDialog::new()
            .set_title(title)
            .set_description(question)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes
    }

    fn open_file(&self, title: &str, kind: &str, extensions: &[&str]) -> Option<PathBuf> {
        file_dialog(title, kind, extensions).pick_file()
    }

    fn save_file(&self, title: &str, name: &str, kind: &str, extensions: &[&str]) -> Option<PathBuf> {
        file_dialog(title, kind, extensions).set_file_name(name).save_file()
    }
}

/// A file dialog that starts in the Documents folder.
fn file_dialog(title: &str, kind: &str, extensions: &[&str]) -> rfd::FileDialog {
    let dialog = rfd::FileDialog::new().set_title(title).add_filter(kind, extensions);
    match dirs::document_dir().or_else(dirs::home_dir) {
        Some(dir) => dialog.set_directory(dir),
        None => dialog,
    }
}

/// "report.pdf in /Users/me/Documents", for the status bar.
fn file_and_folder(path: &Path) -> String {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    match path.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(folder) => format!("{name} in {}", folder.display()),
        None => name,
    }
}

/// Where the app keeps things, for the System Settings tab to tell people.
#[derive(Debug, Clone, Default)]
pub struct Paths {
    /// `None` when the database couldn't be opened and nothing will be kept.
    pub database: Option<PathBuf>,
    pub log: Option<PathBuf>,
}

struct Status {
    text: String,
    error: bool,
}

pub struct App {
    /// For other threads to ask for the window to be drawn again.
    ctx: egui::Context,
    db: Db,
    fmt: LocaleFormat,
    dialogs: Box<dyn Dialogs>,
    paths: Paths,
    tab: Tab,
    status: Status,
    /// A control waiting to be given the keyboard focus, by name.
    focus_wanted: Option<&'static str>,
    /// Escape was pressed, and hasn't been acted on yet.
    escape_pressed: bool,
    /// F1 has replaced the selected tab's controls with the keyboard shortcuts.
    shortcuts_open: bool,
    focus_before_shortcuts: Option<egui::Id>,
    appearance: Appearance,

    /// Every category, oldest first.
    categories: Vec<Category>,

    // Spending: adding it
    name_input: String,
    amount_input: String,
    date_input: String,
    category_index: usize,

    // Spending and Summary, which show the same month
    /// The first day of the month on show.
    month: NaiveDate,
    entries: Vec<Spending>,
    totals: Vec<CategoryTotal>,
    entry_to_delete: usize,

    // Categories
    new_category: String,

    // Import/Export
    /// All spending, or the month on show.
    export_scope: usize,
    /// One of `import_export::ExportFormat::ALL`.
    export_format: usize,

    // System Settings
    locale_input: String,
    /// Whether to ask GitHub for the latest version when the app starts.
    check_updates: bool,
    /// A newer version that GitHub has, once a check has found one.
    newer_version: Option<crate::update::Version>,
    checking_for_updates: bool,
    /// The download page should open in the web browser on this frame.
    open_downloads: bool,
    update_tx: Sender<updates::UpdateResult>,
    update_rx: Receiver<updates::UpdateResult>,
}

/// Today, in the computer's time zone.
pub(crate) fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// "1 entry", "2 entries".
fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

impl App {
    pub fn new(ctx: &egui::Context, db: Db, fmt: LocaleFormat, dialogs: Box<dyn Dialogs>, paths: Paths, problem: String) -> Self {
        let mut problems = vec![problem];
        let appearance = match db.setting("appearance") {
            Ok(saved) => saved.as_deref().and_then(Appearance::from_key).unwrap_or_default(),
            Err(e) => {
                problems.push(format!("Your settings could not be read: {e}."));
                Appearance::default()
            }
        };
        appearance::apply(ctx, appearance);
        ctx.all_styles_mut(|style| {
            style.spacing.interact_size.y = widgets::CONTROL_HEIGHT;
            style.spacing.button_padding = egui::vec2(10.0, 6.0);
            style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
            style.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
            style.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(24.0));
        });
        let locale_input = db.setting("locale").ok().flatten().unwrap_or_default();
        // On unless turned off.
        let check_updates = db.setting("check_updates").ok().flatten().as_deref() != Some("0");
        let (update_tx, update_rx) = channel();
        let mut app = Self {
            ctx: ctx.clone(),
            db,
            fmt,
            dialogs,
            paths,
            // Where spending is added, the most used.
            tab: Tab::Spending,
            status: Status { text: "Ready.".to_owned(), error: false },
            focus_wanted: Some(Tab::Spending.name()),
            escape_pressed: false,
            shortcuts_open: false,
            focus_before_shortcuts: None,
            appearance,
            categories: Vec::new(),
            name_input: String::new(),
            amount_input: String::new(),
            date_input: String::new(),
            category_index: 0,
            month: today().with_day(1).expect("every month has a first day"),
            entries: Vec::new(),
            totals: Vec::new(),
            entry_to_delete: 0,
            new_category: String::new(),
            export_scope: 0,
            export_format: 0,
            locale_input,
            check_updates,
            newer_version: None,
            checking_for_updates: false,
            open_downloads: false,
            update_tx,
            update_rx,
        };
        app.refresh();
        let problem = problems.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ");
        if !problem.is_empty() {
            app.set_error(problem);
        }
        app
    }

    /// Reads the categories and the month on show from the database again.
    fn refresh(&mut self) {
        let next = self.month + chrono::Months::new(1);
        let result = (|| -> rusqlite::Result<_> {
            Ok((self.db.categories()?, self.db.spending_between(self.month, next)?, self.db.totals_between(self.month, next)?))
        })();
        match result {
            Ok((categories, entries, totals)) => {
                self.categories = categories;
                self.entries = entries;
                self.totals = totals;
            }
            Err(e) => {
                log::error!("could not read the database: {e}");
                self.set_error(format!("Your spending could not be read: {e}."));
            }
        }
        self.category_index = self.category_index.min(self.categories.len().saturating_sub(1));
        self.entry_to_delete = self.entry_to_delete.min(self.entries.len().saturating_sub(1));
    }

    /// Puts a message in the status bar, which screen readers announce. They
    /// ignore a live region whose text hasn't changed, so a repeated message
    /// gets a no-break space added to make it differ.
    fn announce(&mut self, text: String, error: bool) {
        // Not the words, which can say what was bought and for how much.
        log::debug!("status: {}", if error { "a problem" } else { "a message" });
        let mut text = text;
        if text == self.status.text.trim_end_matches('\u{a0}') && !self.status.text.ends_with('\u{a0}') {
            text.push('\u{a0}');
        }
        self.status = Status { text, error };
    }

    fn set_info(&mut self, text: impl Into<String>) {
        self.announce(text.into(), false);
    }

    fn set_error(&mut self, text: impl Into<String>) {
        self.announce(text.into(), true);
    }

    /// Saves a setting, reporting any failure.
    fn save_setting(&mut self, name: &str, value: Option<&str>) {
        if let Err(e) = self.db.set_setting(name, value) {
            log::error!("could not save the setting {name}: {e}");
            self.set_error(format!("Your settings could not be saved: {e}."));
        }
    }

    /// An entry as a sentence, such as "Entry 5: Tea, £2.50, on Wednesday
    /// 18 November 2026, in Food and Drink."
    fn describe(&self, e: &Spending) -> String {
        format!(
            "Entry {}: {}, {}, on {}, in {}.",
            e.line,
            e.name,
            self.fmt.money(e.pence),
            self.fmt.long_date(e.date),
            e.category
        )
    }

    /// Whether `name` is waiting for the focus. Asking takes the request away.
    fn wants_focus(&mut self, name: &str) -> bool {
        if self.focus_wanted == Some(name) {
            self.focus_wanted = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn switch_tab(&mut self, tab: Tab) {
        if self.tab != tab {
            log::debug!("switched to tab {tab:?}");
            self.tab = tab;
        }
        self.shortcuts_open = false;
        self.focus_before_shortcuts = None;
        self.focus_wanted = Some(tab.name());
    }

    fn tab_bar(&mut self, ui: &mut Ui) {
        // So screen readers can say "tab 1 of 5".
        ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
            node.set_role(Role::TabList);
            node.set_label("Tabs");
        });
        ui.add_space(8.0);
        for tab in Tab::ALL {
            let selected = self.tab == tab;
            let button = egui::Button::selectable(selected, RichText::new(tab.label()).size(15.0))
                .min_size(egui::vec2(ui.available_width(), 34.0));
            let resp = widgets::stable_scope(ui, tab.name(), |ui| ui.add(button));
            let shortcut = ui.ctx().format_shortcut(&shortcuts::tab(tab));
            ui.ctx().accesskit_node_builder(resp.id, |node| {
                node.set_role(Role::Tab);
                node.set_label(tab.name());
                node.set_selected(selected);
                node.set_keyboard_shortcut(shortcut.clone());
            });
            if self.wants_focus(tab.name()) {
                resp.request_focus();
            }
            if resp.on_hover_text(shortcut).clicked() {
                self.switch_tab(tab);
            }
        }
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.add_space(2.0);
        let colour = if self.status.error { ui.visuals().error_fg_color } else { ui.visuals().text_color() };
        let resp = ui.add(egui::Label::new(RichText::new(&self.status.text).color(colour)).wrap());
        // A mistake interrupts whatever the screen reader is saying.
        let live = if self.status.error { Live::Assertive } else { Live::Polite };
        ui.ctx().accesskit_node_builder(resp.id, |node| {
            node.set_live(live);
        });
        ui.add_space(2.0);
    }

    /// Everything inside the window, once a frame.
    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.handle_update_results();
        if std::mem::take(&mut self.open_downloads) {
            ctx.open_url(egui::OpenUrl::new_tab(crate::update::RELEASES_URL));
        }
        self.handle_shortcuts(&ctx);
        egui::Panel::bottom("status_bar").show(ui, |ui| self.status_bar(ui));
        // The tabs give up some room when space is short, as when text is made
        // much larger, so the controls beside them stay usable.
        let tabs_width = (ui.available_width() * 0.34).clamp(110.0, 190.0);
        egui::Panel::left("tabs").resizable(false).exact_size(tabs_width).show(ui, |ui| self.tab_bar(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                // Keep clear of the scroll bar, so it doesn't cover the controls.
                ui.set_max_width(ui.available_width() - 14.0);
                ui.spacing_mut().item_spacing.y = 8.0;
                match self.tab {
                    _ if self.shortcuts_open => self.shortcuts_view(ui),
                    Tab::Summary => self.summary_tab(ui),
                    Tab::Spending => self.spending_tab(ui),
                    Tab::Categories => self.categories_tab(ui),
                    Tab::ImportExport => self.import_export_tab(ui),
                    Tab::Settings => self.settings_tab(ui),
                }
            });
        });
        // A focus request made while drawing lands on the next frame.
        if self.focus_wanted.is_some() {
            ctx.request_repaint();
        }
        widgets::draw_focus_ring(&ctx);
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.take_escape(raw_input);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}
