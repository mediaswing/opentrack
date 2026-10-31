//! The Settings tab: the language, region, financial year and currency
//! symbol, the debug log, keyboard shortcuts, asking before deleting and
//! update checks.

use super::*;
use std::path::PathBuf;

impl BudgetApp {
    /// Asks GitHub for a newer release on another thread. `requested` is
    /// true when the check was asked for, so its result is always said.
    pub(super) fn start_update_check(&mut self, ctx: &egui::Context, requested: bool) {
        if self.update_check.is_some() {
            return;
        }
        log::info!("checking for updates");
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        crate::update::check_in_background(move |result| {
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.update_check = Some((rx, requested));
        if requested {
            self.announce(t!("update-checking"));
        }
    }

    /// Reports the update check once it has an answer.
    pub(super) fn finish_update_check(&mut self) {
        let Some((rx, requested)) = &self.update_check else { return };
        let requested = *requested;
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(t!("update-failed")),
        };
        self.update_check = None;
        let release = match result {
            Ok(Some(release)) => release,
            Ok(None) => {
                log::info!("no update available");
                if requested {
                    self.announce(t!("update-latest", version = env!("CARGO_PKG_VERSION")));
                }
                return;
            }
            // A check made at start-up fails quietly (the error is in the log).
            Err(e) if requested => return self.show_error(e),
            Err(_) => return,
        };
        log::info!("update available: {}", release.version);
        let question = format!(
            "{}\n\n{}",
            t!("update-available", new = &release.version, current = env!("CARGO_PKG_VERSION")),
            t!("update-open-question")
        );
        let open = rfd::MessageDialog::new()
            .set_title(t!("update-title"))
            .set_description(question)
            .set_buttons(rfd::MessageButtons::YesNo)
            .set_level(rfd::MessageLevel::Info)
            .show()
            == rfd::MessageDialogResult::Yes;
        if !open {
            return self.announce(t!("update-later", version = &release.version));
        }
        match crate::paths::open_url(&release.url) {
            Ok(()) => self.announce(t!("update-opened", version = &release.version)),
            Err(e) => self.show_error(t!("error-open-browser", error = e.to_string(), url = &release.url)),
        }
    }

    /// Shows the interface in another language, or the system's for `None`.
    fn use_language(&mut self, tag: Option<&str>) {
        self.settings.language = tag.map(str::to_owned);
        self.settings.save();
        let language = crate::i18n::resolve(tag);
        crate::i18n::set(language);
        log::info!("language {}", language.tag);
        self.announce(t!("settings-language-changed"));
    }

    /// Uses another region, or the system's for `None`. The date in the
    /// entry form is rewritten so it still reads the same.
    fn use_region(&mut self, tag: Option<&str>) {
        let old = self.region;
        self.settings.region = tag.map(str::to_owned);
        self.settings.save();
        self.region = crate::locale::resolve(tag);
        log::info!("region {}", self.region.tag);
        self.spending.rewrite_date(&old, &self.region);
        self.write_period_dates();
        if self.settings.currency.is_none() {
            self.currency_input = self.region.currency.to_owned();
        }
        // Budgets in the Categories tab are shown again in the new way.
        self.category_form.reload();
        self.reload();
        self.announce(t!(
            "settings-region-changed",
            pattern = self.region.date_pattern(),
            example = self.region.example_date(),
            amount = self.money(123_456)
        ));
    }

    /// Starts the financial year on another day, or the region's usual day
    /// for `None`.
    fn use_year_start(&mut self, start: Option<YearStart>) {
        self.settings.year_start = start;
        self.settings.save();
        let date = self.region.day_and_month(self.year_start());
        self.announce(t!("settings-year-changed", date = date));
    }

    /// Starts or stops the debug log to match the settings.
    fn apply_debug_log(&mut self) {
        if !self.settings.debug_log {
            log::info!("debug log turned off");
            crate::logging::stop();
            return self.announce(t!("log-off"));
        }
        let dir = self.settings.log_dir();
        match crate::logging::start(&dir) {
            Ok(file) => {
                log::info!("debug log turned on, Open Budget Tracker {}", env!("CARGO_PKG_VERSION"));
                self.announce(t!("log-writing", path = file.display().to_string()));
            }
            Err(e) => {
                self.settings.debug_log = false;
                self.settings.save();
                self.show_error(t!("error-start-log", dir = dir.display().to_string(), error = e.to_string()));
            }
        }
    }

    fn use_log_dir(&mut self, dir: PathBuf) {
        if !dir.is_absolute() {
            self.focus = Some(Id::new("log_dir"));
            return self.show_error(t!("log-dir-not-absolute"));
        }
        self.log_dir_input = dir.display().to_string();
        self.settings.log_dir = Some(dir);
        self.settings.save();
        if self.settings.debug_log {
            self.apply_debug_log();
        } else {
            self.announce(t!("log-dir-saved"));
        }
    }

    pub(super) fn settings_tab(&mut self, ui: &mut Ui) {
        heading(ui, &t!("settings-language-heading"));
        let detected = crate::i18n::system();
        let system = crate::i18n::system_tags().into_iter().next().unwrap_or_default();
        let mut options = vec![t!("settings-language-automatic", language = detected.name, system = &system)];
        options.extend(crate::i18n::LANGUAGES.iter().map(|l| l.name.to_owned()));
        let mut index = self
            .settings
            .language
            .as_deref()
            .and_then(crate::i18n::find)
            .and_then(|chosen| crate::i18n::LANGUAGES.iter().position(|l| l.tag == chosen.tag))
            .map_or(0, |i| i + 1);
        if dropdown(ui, "language", &t!("settings-language-label"), &options, &mut index, true) {
            let tag = index.checked_sub(1).map(|i| crate::i18n::LANGUAGES[i].tag);
            self.use_language(tag);
        }

        heading(ui, &t!("settings-region-heading"));
        let system = crate::locale::system_tag();
        let detected = crate::locale::for_tag(&system);
        let mut options = vec![t!("settings-region-automatic", region = detected.name(), system = &system)];
        options.extend(crate::locale::REGIONS.iter().map(|r| r.name()));
        let mut index = self
            .settings
            .region
            .as_deref()
            .and_then(|tag| crate::locale::REGIONS.iter().position(|r| r.tag == tag))
            .map_or(0, |i| i + 1);
        if dropdown(ui, "region", &t!("settings-region-label"), &options, &mut index, true) {
            let tag = index.checked_sub(1).map(|i| crate::locale::REGIONS[i].tag);
            self.use_region(tag);
        }
        let today = self.region.long_date(crate::money::today());
        note(
            ui,
            &t!(
                "settings-dates-note",
                pattern = self.region.date_pattern(),
                example = self.region.example_date(),
                today = today
            ),
        );

        heading(ui, &t!("settings-year-heading"));
        let start = self.year_start();
        let days: Vec<String> = (1..=31).map(|d: u32| d.to_string()).collect();
        let months: Vec<String> = (1..=12).map(|m| crate::i18n::message(&format!("month-{m}"), None)).collect();
        let (mut day, mut month) = (start.day as usize - 1, start.month as usize - 1);
        let mut changed = false;
        ui.columns(2, |cols| {
            changed |= dropdown(&mut cols[0], "year_start_day", &t!("settings-year-day-label"), &days, &mut day, true);
            changed |= dropdown(&mut cols[1], "year_start_month", &t!("settings-year-month-label"), &months, &mut month, true);
        });
        if changed {
            self.use_year_start(Some(YearStart { month: month as u32 + 1, day: day as u32 + 1 }));
        }
        let year = self.region.period_label(self.year_start().year_of(crate::money::today()));
        note(ui, &t!("settings-year-note", period = year, choice = t!("period-choose-dates")));
        let usual = t!("settings-usual-year", date = self.region.day_and_month(self.region.year_start()));
        if full_button(ui, &usual, self.settings.year_start.is_some()).clicked() {
            self.use_year_start(None);
        }

        heading(ui, &t!("settings-money-heading"));
        let was_usable = crate::locale::is_usable_symbol(self.currency_input.trim());
        let resp = text_field(
            ui,
            Id::new("currency"),
            &t!("settings-currency-label"),
            &mut self.currency_input,
            &t!("settings-currency-hint"),
        );
        if resp.changed() {
            self.currency_input = self.currency_input.chars().take(4).collect();
            let symbol = self.currency_input.trim();
            if crate::locale::is_usable_symbol(symbol) {
                self.settings.currency = Some(symbol.to_owned());
                self.currency_unsaved = true;
            } else if was_usable {
                // Said once as the symbol becomes unusable, not on every key.
                self.announce(t!("settings-currency-refused", symbol = symbol));
            }
        }
        if resp.lost_focus() && self.currency_unsaved {
            self.settings.save();
            self.currency_unsaved = false;
        }
        let typed = self.currency_input.trim();
        if crate::locale::is_usable_symbol(typed) {
            note(ui, &t!("settings-amounts-shown", amount = self.money(123_456)));
        } else {
            note(ui, &t!("settings-currency-refused-note", symbol = typed, amount = self.money(123_456)));
        }
        let usual = self.region.currency;
        let usual_label =
            if usual.is_empty() { t!("settings-no-symbol") } else { t!("settings-usual-symbol", symbol = usual) };
        if full_button(ui, &usual_label, self.settings.currency.is_some()).clicked() {
            self.settings.currency = None;
            self.settings.save();
            self.currency_unsaved = false;
            self.currency_input = usual.to_owned();
            self.announce(t!("settings-amounts-shown", amount = self.money(123_456)));
        }

        heading(ui, &t!("settings-keyboard-heading"));
        if ui.checkbox(&mut self.settings.shortcuts, t!("settings-shortcuts-checkbox")).changed() {
            self.keymap = Keymap::new(self.settings.shortcuts, Platform::CURRENT);
            self.settings.save();
            self.announce(if self.settings.shortcuts { t!("settings-shortcuts-on") } else { t!("settings-shortcuts-off") });
        }
        let list = format!("{}{}", t!("settings-list-shortcuts"), self.keymap.hint(Action::Help));
        if full_button(ui, &list, true).clicked() {
            self.show_shortcuts();
        }

        heading(ui, &t!("settings-deleting-heading"));
        if ui.checkbox(&mut self.settings.confirm_delete, t!("settings-confirm-delete")).changed() {
            self.settings.save();
        }

        heading(ui, &t!("settings-updates-heading"));
        note(ui, &t!("settings-updates-note"));
        if ui.checkbox(&mut self.settings.check_updates, t!("settings-check-at-start")).changed() {
            self.settings.save();
        }
        if full_button(ui, &t!("settings-check-now"), self.update_check.is_none()).clicked() {
            let ctx = ui.ctx().clone();
            self.start_update_check(&ctx, true);
        }

        heading(ui, &t!("settings-log-heading"));
        note(ui, &t!("settings-log-note"));
        if ui.checkbox(&mut self.settings.debug_log, t!("settings-log-checkbox")).changed() {
            self.settings.save();
            self.apply_debug_log();
        }
        let label = t!("settings-log-dir-label");
        let resp = text_field(ui, Id::new("log_dir"), &label, &mut self.log_dir_input, &t!("settings-log-dir-hint"));
        let entered = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        let (use_folder, choose, open) = (t!("settings-use-folder"), t!("settings-choose-folder"), t!("settings-open-folder"));
        match button_row(ui, &[(&use_folder, true), (&choose, true), (&open, true)]) {
            Some(0) => self.use_log_dir(PathBuf::from(self.log_dir_input.trim())),
            Some(1) => {
                if let Some(dir) = rfd::FileDialog::new().set_directory(self.settings.log_dir()).pick_folder() {
                    self.use_log_dir(dir);
                }
            }
            Some(_) => {
                let dir = self.settings.log_dir();
                let opened = crate::paths::create_private_dir(&dir).and_then(|()| crate::paths::open_folder(&dir));
                if let Err(e) = opened {
                    self.show_error(t!("error-open-folder", dir = dir.display().to_string(), error = e.to_string()));
                }
            }
            None if entered => self.use_log_dir(PathBuf::from(self.log_dir_input.trim())),
            None => {}
        }

        heading(ui, &t!("settings-about-heading"));
        note(ui, &t!("settings-about", version = env!("CARGO_PKG_VERSION")));
    }
}
