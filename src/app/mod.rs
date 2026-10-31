//! The window: five tabs (Spending, Summary, Categories, Import/Export,
//! Settings) with every control on its own full-width line, laid out as in
//! the Speech Output Engine.
//!
//! Accessibility notes:
//! * egui publishes the interface to screen readers through AccessKit
//!   (UI Automation on Windows, NSAccessibility on macOS, AT-SPI on Linux).
//! * Every text field and dropdown is linked to its visible label, the tabs
//!   are exposed with the tab role and selected state, entries are list
//!   options, and the status line is a polite live region so what happened
//!   is announced.
//! * Everything works from the keyboard: Tab and Shift+Tab move between
//!   controls, Space or Enter activates them, arrow keys change a focused
//!   dropdown and move through the entries, and the shortcuts in
//!   `shortcuts.rs` work from anywhere.
//! * A thick focus ring is drawn around whichever control has focus.

mod categories_tab;
mod data_tab;
mod keys;
mod settings_tab;
mod spending_tab;
mod summary_tab;
mod widgets;

use crate::APP_TITLE;
use crate::db::{Category, Db};
use crate::locale::Region;
use crate::money::{self, Month, Period, YearStart};
use crate::settings::Settings;
use crate::shortcuts::{Action, Command, KeyContext, Keymap, Platform};
use crate::update::Release;
use egui::accesskit::{Live, Role};
use egui::{Button, Color32, EventFilter, Id, Key, Modifiers, RichText, Ui};
use categories_tab::CategoryForm;
use spending_tab::SpendingTab;
use summary_tab::Summary;
use std::sync::mpsc::Receiver;
use widgets::*;

const CONTROL_HEIGHT: f32 = 36.0;

/// What an update check found: a newer release, none, or why it failed.
type UpdateResult = Result<Option<Release>, String>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Spending,
    Summary,
    Categories,
    Data,
    Settings,
}

impl Tab {
    const ALL: [Tab; 5] = [Tab::Spending, Tab::Summary, Tab::Categories, Tab::Data, Tab::Settings];

    fn label(self) -> String {
        match self {
            Tab::Spending => t!("tab-spending"),
            Tab::Summary => t!("tab-summary"),
            Tab::Categories => t!("tab-categories"),
            Tab::Data => t!("tab-data"),
            Tab::Settings => t!("tab-settings"),
        }
    }

    fn id(self) -> Id {
        Id::new(("tab", self as u8))
    }
}

pub struct BudgetApp {
    db: Db,
    /// Where the database is.
    db_path: std::path::PathBuf,
    settings: Settings,
    /// How dates and amounts are written.
    region: Region,
    tab: Tab,
    /// Every category, in alphabetical order.
    categories: Vec<Category>,
    /// How many entries each category has, by its id. Read in `reload`
    /// rather than every frame.
    entry_counts: std::collections::HashMap<i64, i64>,
    /// The dates the Spending and Summary tabs show.
    period: Period,
    /// The From and To dates as typed, when showing a run of days.
    period_first: String,
    period_last: String,
    spending: SpendingTab,
    summary: Summary,
    category_form: CategoryForm,
    currency_input: String,
    /// Whether the currency symbol has changed since the settings were last
    /// saved. It is saved when its box loses focus, not on every key.
    currency_unsaved: bool,
    log_dir_input: String,
    /// The Settings tab's choice of database: 0 for the built-in one, 1
    /// for a server. It only changes the database when the user connects.
    database_choice: usize,
    /// The server's details as typed on the Settings tab.
    server_form: crate::settings::Server,
    server_port_input: String,
    status: String,
    /// A tab button to give focus to.
    focus_tab: Option<Tab>,
    /// A control to give focus to, by its id.
    focus: Option<Id>,
    keymap: Keymap,
    /// Shortcuts pressed since the last frame, taken from the input before
    /// egui saw it.
    pending: Vec<Command>,
    /// The tab buttons' ids last frame, to tell when one has focus.
    tab_buttons: Vec<Id>,
    /// The window has been checked against the screen size.
    fitted_to_screen: bool,
    /// The answer to an update check that is running, and whether it was
    /// asked for rather than made when the app started.
    update_check: Option<(Receiver<UpdateResult>, bool)>,
}

impl BudgetApp {
    pub fn new(
        ctx: &egui::Context,
        db: Db,
        db_path: std::path::PathBuf,
        settings: Settings,
        start_status: String,
    ) -> Self {
        setup_style(ctx);
        let keymap = Keymap::new(settings.shortcuts, Platform::CURRENT);
        let database_choice = usize::from(db.is_on_server());
        let mut app = Self {
            db,
            db_path,
            region: crate::locale::resolve(settings.region.as_deref()),
            currency_input: String::new(),
            currency_unsaved: false,
            log_dir_input: settings.log_dir().display().to_string(),
            database_choice,
            server_form: settings.server.clone(),
            server_port_input: settings.server.port.to_string(),
            settings,
            tab: Tab::Spending,
            categories: Vec::new(),
            entry_counts: Default::default(),
            period: Period::Month(Month::current()),
            period_first: String::new(),
            period_last: String::new(),
            spending: SpendingTab::default(),
            summary: Summary::default(),
            category_form: CategoryForm::default(),
            status: String::new(),
            focus_tab: Some(Tab::Spending),
            focus: None,
            keymap,
            pending: Vec::new(),
            tab_buttons: Vec::new(),
            fitted_to_screen: false,
            update_check: None,
        };
        log::info!("region {} (system {})", app.region.tag, crate::locale::system_tag());
        log::info!("languages (system {:?})", crate::i18n::system_tags());
        app.currency_input = app.currency().to_owned();
        app.reload();
        app.spending.clear_form(&app.region);
        let help = app.keymap.hint(Action::Help);
        let ready = if help.is_empty() {
            t!("status-ready")
        } else {
            t!("status-ready-help", key = help.trim().trim_matches(['(', ')']))
        };
        app.status = format!("{ready} {start_status}").trim_end().to_owned();
        if app.settings.check_updates {
            app.start_update_check(ctx, false);
        }
        app
    }

    /// Reads the categories, entries and summary again after a change.
    fn reload(&mut self) {
        match self.db.categories() {
            Ok(categories) => self.categories = categories,
            Err(e) => self.show_error(t!("error-read-categories", error = format!("{e:#}"))),
        }
        match self.db.entry_counts() {
            Ok(counts) => self.entry_counts = counts.into_iter().collect(),
            Err(e) => self.show_error(t!("error-count-entries", error = format!("{e:#}"))),
        }
        self.reload_entries();
        self.reload_summary();
        self.category_form.sync(&self.categories, &self.region);
    }

    /// After the whole budget has changed underneath the window, such as
    /// from a backup or another database: nothing being changed, chosen or
    /// typed still refers to what was there before.
    fn fresh_start(&mut self) {
        self.spending.clear_form(&self.region);
        self.spending.selected = None;
        self.category_form = CategoryForm::default();
        self.reload();
    }

    fn money(&self, pence: i64) -> String {
        self.region.money(pence, self.currency())
    }

    /// The currency symbol: the one chosen, or the region's. A chosen one
    /// that can't be used, from an older version's settings, is ignored.
    fn currency(&self) -> &str {
        self.settings.currency.as_deref().filter(|s| crate::locale::is_usable_symbol(s)).unwrap_or(self.region.currency)
    }

    /// When the financial year starts: as chosen, or the region's usual day.
    fn year_start(&self) -> YearStart {
        self.settings.year_start.filter(|s| s.is_valid()).unwrap_or_else(|| self.region.year_start())
    }

    /// Reads an amount as typed.
    fn parse_amount(&self, text: &str) -> Result<i64, String> {
        self.region.parse_amount(text, self.currency())
    }

    fn category_name(&self, id: i64) -> String {
        self.categories.iter().find(|c| c.id == id).map_or_else(|| t!("category-unknown"), |c| c.short.clone())
    }

    fn announce(&mut self, text: impl Into<String>) {
        // Not logged: messages can say what was bought and for how much.
        let mut text = text.into();
        // Screen readers ignore a live region whose text has not changed, so
        // a repeated message would be silent. A trailing no-break space makes
        // the text differ.
        if text == self.status.trim_end_matches('\u{a0}') && !self.status.ends_with('\u{a0}') {
            text.push('\u{a0}');
        }
        self.status = text;
    }

    /// Reports something that went wrong: in the status line, and in an
    /// error dialog so it cannot be missed. Screen readers read the dialog
    /// out when it opens.
    ///
    /// The message can quote amounts, names and categories, so the log only
    /// records where it came from.
    #[track_caller]
    fn show_error(&mut self, text: impl Into<String>) {
        let text = text.into();
        log::warn!("error shown, from {}", std::panic::Location::caller());
        self.announce(text.clone());
        rfd::MessageDialog::new()
            .set_title(APP_TITLE)
            .set_description(text)
            .set_buttons(rfd::MessageButtons::Ok)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }

    /// Asks a yes or no question in a dialog.
    fn confirm(&self, title: &str, question: &str) -> bool {
        rfd::MessageDialog::new()
            .set_title(title)
            .set_description(question)
            .set_buttons(rfd::MessageButtons::YesNo)
            .set_level(rfd::MessageLevel::Warning)
            .show()
            == rfd::MessageDialogResult::Yes
    }

    /// The window opens at its usual size, which can be taller than the room
    /// above the taskbar on a small or zoomed screen, hiding the status
    /// line. Once the screen size is known, shrink the window to fit and
    /// move it up if its bottom would be covered.
    fn fit_to_screen(&mut self, ctx: &egui::Context) {
        /// Room left for a taskbar, dock or panel.
        const RESERVED: f32 = 56.0;
        if self.fitted_to_screen {
            return;
        }
        let (monitor, inner, outer) = ctx.input(|i| {
            let v = i.viewport();
            (v.monitor_size, v.inner_rect, v.outer_rect)
        });
        // Some systems (Wayland) never report the window's position.
        let (Some(monitor), Some(inner), Some(outer)) = (monitor, inner, outer) else { return };
        self.fitted_to_screen = true;
        let border = outer.size() - inner.size();
        let largest = (monitor - border - egui::vec2(0.0, RESERVED)).max(egui::vec2(480.0, 480.0));
        let size = inner.size().min(largest);
        if size != inner.size() {
            log::info!("shrinking the window from {:?} to {size:?} to fit the screen", inner.size());
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        }
        // Only move a window on the main screen, whose top-left corner is 0, 0.
        let bottom = outer.top() + border.y + size.y;
        if outer.top() >= 0.0 && outer.top() < monitor.y && bottom > monitor.y - RESERVED {
            let top = (monitor.y - RESERVED - border.y - size.y).max(0.0);
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(outer.left(), top)));
        }
    }

    fn switch_tab(&mut self, tab: Tab) {
        if self.tab != tab {
            log::debug!("switching to the {tab:?} tab");
        }
        self.tab = tab;
        self.focus_tab = Some(tab);
    }

    fn tab_bar(&mut self, ui: &mut Ui) {
        self.tab_buttons.clear();
        ui.columns(Tab::ALL.len(), |cols| {
            for (col, tab) in cols.iter_mut().zip(Tab::ALL) {
                let selected = self.tab == tab;
                let text = RichText::new(tab.label()).family(tab_font()).size(18.0);
                let text = if selected { text.strong() } else { text };
                let resp = col
                    .push_id(tab.id(), |ui| {
                        ui.add_sized([ui.available_width(), 40.0], Button::selectable(selected, text))
                    })
                    .inner;
                self.tab_buttons.push(resp.id);
                col.ctx().accesskit_node_builder(resp.id, |node| {
                    node.set_role(Role::Tab);
                    node.set_selected(selected);
                });
                if self.focus_tab == Some(tab) {
                    resp.request_focus();
                    self.focus_tab = None;
                }
                if resp.clicked() && !selected {
                    self.switch_tab(tab);
                    self.announce(t!("tab-announce", tab = tab.label()));
                }
            }
        });
    }

    fn status_bar(&self, ui: &mut Ui) {
        let resp = ui.add(egui::Label::new(RichText::new(&self.status).size(16.0)).wrap());
        ui.ctx().accesskit_node_builder(resp.id, |node| {
            node.set_live(Live::Polite);
        });
    }
}

impl eframe::App for BudgetApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.take_shortcuts(ctx, raw_input);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // A symbol still being typed when the window closes.
        if self.currency_unsaved {
            self.settings.save();
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }
}

impl BudgetApp {
    /// Draws the window and does what was asked since the last frame.
    fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.fit_to_screen(&ctx);
        self.run_shortcuts(&ctx);
        self.finish_update_check();

        egui::Panel::top("tabs").show(ui, |ui| {
            ui.add_space(4.0);
            self.tab_bar(ui);
            ui.add_space(4.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.add_space(6.0);
            self.status_bar(ui);
            ui.add_space(4.0);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                match self.tab {
                    Tab::Spending => self.spending_tab(ui),
                    Tab::Summary => self.summary_tab(ui),
                    Tab::Categories => self.categories_tab(ui),
                    Tab::Data => self.data_tab(ui),
                    Tab::Settings => self.settings_tab(ui),
                }
            });
        });
        // After the controls are laid out, so the one asked for exists.
        if let Some(id) = self.focus.take() {
            ctx.memory_mut(|m| m.request_focus(id));
        }

        draw_focus_ring(&ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, RawInput};

    /// Runs the whole window without a screen, as eframe would: shortcuts
    /// are taken from the input first, then a frame is drawn.
    struct Harness {
        ctx: egui::Context,
        app: BudgetApp,
    }

    impl Harness {
        fn new() -> Self {
            Self::in_region("en-GB")
        }

        fn in_region(region: &str) -> Self {
            let ctx = egui::Context::default();
            let db = Db::open_in_memory().unwrap();
            // No dialogs, which would wait for someone to answer, no update
            // check, which would go to GitHub, and the same region whatever
            // the computer is set to.
            let settings = Settings {
                confirm_delete: false,
                check_updates: false,
                region: Some(region.into()),
                ..Settings::default()
            };
            let app = BudgetApp::new(&ctx, db, "test.db".into(), settings, String::new());
            let mut h = Harness { ctx, app };
            h.frame(vec![]);
            h.frame(vec![]);
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            // Tall enough that every control is on screen.
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 3000.0));
            let mut input = RawInput { events, screen_rect: Some(screen), ..Default::default() };
            self.app.take_shortcuts(&self.ctx, &mut input);
            let app = &mut self.app;
            let mut output = self.ctx.run_ui(input, |ui| app.show(ui));
            output.textures_delta.clear();
        }

        /// Presses a key. `Modifiers::COMMAND` is Cmd on a Mac and Ctrl
        /// elsewhere, as when typed: egui sets `mac_cmd` too for Cmd.
        fn press(&mut self, key: Key, mut modifiers: Modifiers) {
            if cfg!(target_os = "macos") && modifiers.command {
                modifiers.mac_cmd = true;
            }
            self.frame(vec![Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }]);
            self.frame(vec![]);
        }

        fn type_text(&mut self, text: &str) {
            self.frame(vec![Event::Text(text.into())]);
        }

        fn focused(&self) -> Option<Id> {
            self.ctx.memory(|m| m.focused())
        }
    }

    #[test]
    fn entries_can_be_added_found_edited_and_deleted_from_the_keyboard() {
        let mut h = Harness::new();
        h.press(Key::N, Modifiers::COMMAND);
        assert_eq!(h.focused(), Some(Id::new(spending_tab::NAME_ID)), "Ctrl+N goes to the name box");
        h.type_text("Lunch");
        h.press(Key::Tab, Modifiers::NONE);
        h.press(Key::Tab, Modifiers::NONE);
        h.type_text("8.50");
        h.press(Key::S, Modifiers::COMMAND);
        let entries = h.app.db.all_entries().unwrap();
        assert_eq!(entries.iter().map(|e| (e.name.as_str(), e.price)).collect::<Vec<_>>(), [("Lunch", 850)]);
        assert!(h.app.status.starts_with("Added Lunch, £8.50"), "{}", h.app.status);
        assert_eq!(h.focused(), Some(Id::new(spending_tab::NAME_ID)), "ready for the next entry");

        // Enter in the form saves too.
        h.type_text("Bus");
        h.press(Key::Tab, Modifiers::NONE);
        h.press(Key::Tab, Modifiers::NONE);
        h.type_text("2");
        h.press(Key::Enter, Modifiers::NONE);
        assert_eq!(h.app.db.all_entries().unwrap().len(), 2);

        // Ctrl+F, then Tab into the list: the newest entry first.
        h.press(Key::F, Modifiers::COMMAND);
        assert_eq!(h.focused(), Some(Id::new(spending_tab::FIND_ID)));
        h.press(Key::Tab, Modifiers::NONE);
        assert_eq!(h.app.spending.selected, Some(2));
        h.press(Key::ArrowDown, Modifiers::NONE);
        h.frame(vec![]);
        assert_eq!(h.app.spending.selected, Some(1), "Down moves to the next entry");
        assert_eq!(h.focused(), Some(Id::new(("entry", 1_i64))));

        h.press(Key::F2, Modifiers::NONE);
        assert!(h.app.spending.is_editing());
        assert_eq!(h.focused(), Some(Id::new(spending_tab::NAME_ID)));
        h.press(Key::Escape, Modifiers::NONE);
        h.frame(vec![]);
        assert!(!h.app.spending.is_editing(), "Escape stops editing");
        assert_eq!(h.focused(), Some(Id::new(("entry", 1_i64))), "and goes back to the entry");

        h.press(Key::D, Modifiers::COMMAND);
        let left = h.app.db.all_entries().unwrap();
        assert_eq!(left.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Bus"]);
    }

    #[test]
    fn entries_are_typed_and_shown_the_regions_way() {
        let mut h = Harness::in_region("de-DE");
        assert_eq!(h.app.spending.date, h.app.region.short_date(money::today()), "today, such as 30.10.2026");
        h.press(Key::N, Modifiers::COMMAND);
        h.type_text("Kaffee");
        h.app.spending.date = "5.3.2026".into();
        h.press(Key::Tab, Modifiers::NONE);
        h.press(Key::Tab, Modifiers::NONE);
        h.type_text("1.234,5");
        h.press(Key::S, Modifiers::COMMAND);
        let entries = h.app.db.all_entries().unwrap();
        let date = chrono::NaiveDate::from_ymd_opt(2026, 3, 5).unwrap();
        assert_eq!(entries.iter().map(|e| (e.date, e.price)).collect::<Vec<_>>(), [(date, 123_450)]);
        assert!(h.app.status.starts_with("Added Kaffee, 1.234,50\u{a0}€"), "{}", h.app.status);
        assert_eq!(h.app.spending.date, "05.03.2026", "the next entry is on the same day");
    }

    #[test]
    fn the_window_can_be_in_german() {
        crate::i18n::with_language("de", || {
            let mut h = Harness::in_region("de-DE");
            assert_eq!(Tab::Spending.label(), "Ausgaben");
            assert!(h.app.categories.iter().any(|c| c.short == "Essen und Trinken"), "starting categories in German");
            h.press(Key::N, Modifiers::COMMAND);
            assert_eq!(h.app.status, "Neuer Eintrag. Geben Sie ein, wofür das Geld ausgegeben wurde.");
            h.type_text("Kaffee");
            h.app.spending.date = "5.3.2026".into();
            h.press(Key::Tab, Modifiers::NONE);
            h.press(Key::Tab, Modifiers::NONE);
            h.type_text("3,5");
            h.press(Key::S, Modifiers::COMMAND);
            assert!(h.app.status.starts_with("Kaffee, 3,50\u{a0}€, in "), "{}", h.app.status);
            assert!(h.app.status.contains(" hinzugefügt."), "{}", h.app.status);
            assert_eq!(h.app.region.long_date(chrono::NaiveDate::from_ymd_opt(2026, 3, 5).unwrap()), "Do., 5. März 2026");
            assert!(h.app.keymap.reference().contains("Tastenkürzel auflisten"));
            assert!(h.app.parse_amount("x").unwrap_err().contains("ist kein Betrag"));
        });
    }

    #[test]
    fn dates_from_one_day_to_another() {
        let mut h = Harness::new();
        let day = |y, m, d| chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap();
        let pets = h.app.db.add_category("Pets", Some(3000)).unwrap();
        for (date, price) in [(day(2026, 4, 5), 100), (day(2026, 4, 6), 200), (day(2027, 4, 5), 400)] {
            let entry = crate::db::Entry { line: 0, name: "Food".into(), date, price, bucket: pets };
            h.app.db.add_entry(&entry).unwrap();
        }
        h.app.reload();

        // The UK tax year, as when "From one date to another" is chosen.
        let year = h.app.year_start().year_of(day(2026, 10, 31));
        h.app.change_period(year);
        assert_eq!((h.app.period_first.as_str(), h.app.period_last.as_str()), ("06/04/2026", "05/04/2027"));
        assert!(h.app.status.starts_with("6 Apr 2026 to 5 Apr 2027: 2 entries, £6.00 spent"), "{}", h.app.status);
        assert_eq!(h.app.summary.spent, 600);
        assert!(h.app.summary_text().starts_with("From 6 Apr 2026 to 5 Apr 2027 you spent £6.00 of your £360.00"));
        h.app.switch_tab(Tab::Summary);
        h.frame(vec![]);
        h.press(Key::F3, Modifiers::NONE);
        assert_eq!(h.app.period_first, "06/04/2025", "F3 goes back a year");
        for tab in [Tab::Spending, Tab::Settings] {
            h.app.switch_tab(tab);
            h.frame(vec![]);
        }

        h.app.period_first = "1/4/26".into();
        h.app.period_last = "5/4/2026".into();
        h.app.show_typed_dates();
        assert_eq!(h.app.period, Period::Days { first: day(2026, 4, 1), last: day(2026, 4, 5) });
        assert_eq!(h.app.summary.spent, 100);
        // 5 of April's 30 days of the £30.00 budget.
        assert_eq!(h.app.summary.budget, 500);
        h.press(Key::F5, Modifiers::NONE);
        assert_eq!(h.app.period, Period::Month(Month::current()), "F5 goes back to this month");
    }

    /// Switching to a server on the Settings tab, using it, and back. The
    /// built-in database has no entries, so no copy is offered (which
    /// would ask in a dialog).
    #[test]
    fn the_budget_can_move_to_a_server_and_back() {
        let Some(test) = crate::db::tests::TestDatabase::new() else { return };
        let mut h = Harness::new();
        let dir = tempfile::tempdir().unwrap();
        h.app.db_path = dir.path().join("budget.db");
        h.app.switch_tab(Tab::Settings);
        h.app.database_choice = 1;
        h.frame(vec![]);

        h.app.server_form = crate::settings::Server { port: 0, ..test.0.clone() };
        h.app.server_port_input = test.0.port.to_string();
        h.app.use_server();
        assert!(h.app.db.is_on_server());
        assert!(h.app.settings.use_server && h.app.settings.server == test.0, "remembered for next time");
        assert!(h.app.status.starts_with(&format!("Now using {} on ", test.0.database)), "{}", h.app.status);
        assert!(h.app.db.location().contains(&test.0.database));

        h.press(Key::N, Modifiers::COMMAND);
        h.type_text("Lunch");
        h.press(Key::Tab, Modifiers::NONE);
        h.press(Key::Tab, Modifiers::NONE);
        h.type_text("8.50");
        h.press(Key::S, Modifiers::COMMAND);
        let on_server = crate::db::Db::connect(&test.0).unwrap().all_entries().unwrap();
        assert_eq!(on_server.iter().map(|e| (e.name.as_str(), e.price)).collect::<Vec<_>>(), [("Lunch", 850)]);

        h.app.use_built_in();
        assert!(!h.app.db.is_on_server() && !h.app.settings.use_server);
        assert!(h.app.db.all_entries().unwrap().is_empty(), "the server's entries stay on the server");
    }

    /// The list's text is made when the entries are read, not every frame,
    /// so it must be made again when the currency symbol changes.
    #[test]
    fn the_list_follows_the_currency_symbol() {
        let mut h = Harness::new();
        let entry = crate::db::Entry { line: 0, name: "Lunch".into(), date: money::today(), price: 850, bucket: 23 };
        h.app.db.add_entry(&entry).unwrap();
        h.app.reload();
        assert_eq!(h.app.spending.shown_prices(), ["£8.50"]);

        h.app.switch_tab(Tab::Settings);
        h.app.currency_input.clear();
        h.frame(vec![]);
        h.ctx.memory_mut(|m| m.request_focus(Id::new("currency")));
        h.frame(vec![]);
        h.type_text("$");
        assert_eq!(h.app.settings.currency.as_deref(), Some("$"));
        assert_eq!(h.app.spending.shown_prices(), ["$8.50"]);
    }

    #[test]
    fn an_entry_being_changed_follows_its_deleted_category() {
        let mut h = Harness::new();
        let (transport, entertainment) = (26, 22);
        let entry = crate::db::Entry { line: 0, name: "Bus".into(), date: money::today(), price: 250, bucket: transport };
        let line = h.app.db.add_entry(&entry).unwrap();
        h.app.reload();
        h.app.spending.selected = Some(line);
        h.app.edit_selected();
        assert!(h.app.spending.is_editing());

        // Deleted on the Categories tab, moving its entries, mid-edit.
        h.app.db.delete_category(transport, Some(entertainment)).unwrap();
        h.app.category_deleted(transport, Some(entertainment));
        assert_eq!(h.app.spending.category, Some(entertainment), "not the first category, Accommodation");
        h.app.save_entry();
        assert_eq!(h.app.db.all_entries().unwrap()[0].bucket, entertainment);
    }

    #[test]
    fn tabs_months_and_categories_from_the_keyboard() {
        let mut h = Harness::new();
        h.press(Key::Num2, Modifiers::COMMAND);
        assert_eq!(h.app.tab, Tab::Summary);
        let month = h.app.period;
        h.press(Key::F3, Modifiers::NONE);
        assert_eq!(h.app.period, month.previous());
        h.press(Key::F5, Modifiers::NONE);
        assert_eq!(h.app.period, month);

        // Ctrl+Tab is Ctrl on a Mac too.
        h.press(Key::Tab, Modifiers::CTRL);
        assert_eq!(h.app.tab, Tab::Categories);
        h.press(Key::N, Modifiers::COMMAND);
        h.type_text("Pets");
        h.press(Key::Tab, Modifiers::NONE);
        h.type_text("50");
        h.press(Key::S, Modifiers::COMMAND);
        let pets = h.app.categories.iter().find(|c| c.short == "Pets").expect("Ctrl+S adds the category");
        assert_eq!(pets.budget, Some(5000));
        assert_eq!(h.app.categories.len(), 9);
    }
}
