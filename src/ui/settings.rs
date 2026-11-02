use std::path::PathBuf;

use egui::RichText;

use super::App;
use crate::config::{self, Backend, Config};
use crate::db;
use crate::locale::Locale;
use crate::update;

pub struct State {
    draft: Config,
    sqlite_path: String,
    port: String,
    test_result: Option<(bool, String)>,
}

impl State {
    pub fn new(cfg: &Config) -> Self {
        Self {
            draft: cfg.clone(),
            sqlite_path: cfg.sqlite_path.display().to_string(),
            port: cfg.mysql.port.to_string(),
            test_result: None,
        }
    }

    /// Build a config from the form, validating text fields.
    fn build(&self) -> Result<Config, String> {
        let mut cfg = self.draft.clone();
        let path = self.sqlite_path.trim();
        if cfg.backend == Backend::Sqlite && path.is_empty() {
            return Err("Enter a path for the SQLite database".into());
        }
        cfg.sqlite_path = PathBuf::from(path);
        if cfg.backend == Backend::MySql {
            cfg.mysql.port = self.port.trim().parse().map_err(|_| format!("'{}' is not a valid port", self.port))?;
            if cfg.mysql.host.trim().is_empty() || cfg.mysql.database.trim().is_empty() {
                return Err("Host and database name are required".into());
            }
        }
        if cfg.currency_override.trim().chars().count() > 4 {
            return Err("Currency symbol should be at most 4 characters".into());
        }
        Ok(cfg)
    }
}

impl App {
    pub(super) fn settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.add_space(10.0);

        egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
            ui.label(RichText::new("Database").strong());
            ui.add_space(4.0);
            let s = &mut self.settings;
            ui.horizontal(|ui| {
                ui.radio_value(&mut s.draft.backend, Backend::Sqlite, "SQLite (built in)");
                ui.radio_value(&mut s.draft.backend, Backend::MySql, "MariaDB / MySQL");
            });
            ui.add_space(6.0);
            egui::Grid::new("db_settings").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| match s.draft.backend {
                Backend::Sqlite => {
                    ui.label("Database file");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut s.sqlite_path).desired_width(420.0))
                            .on_hover_text(&s.sqlite_path);
                        if ui.small_button("Default").clicked() {
                            s.sqlite_path = Config::default().sqlite_path.display().to_string();
                        }
                    });
                    ui.end_row();
                }
                Backend::MySql => {
                    let m = &mut s.draft.mysql;
                    ui.label("Host");
                    ui.add(egui::TextEdit::singleline(&mut m.host).desired_width(240.0));
                    ui.end_row();
                    ui.label("Port");
                    ui.add(egui::TextEdit::singleline(&mut s.port).desired_width(80.0));
                    ui.end_row();
                    ui.label("Database");
                    ui.add(egui::TextEdit::singleline(&mut m.database).desired_width(240.0));
                    ui.end_row();
                    ui.label("User");
                    ui.add(egui::TextEdit::singleline(&mut m.user).desired_width(240.0));
                    ui.end_row();
                    ui.label("Password");
                    ui.add(egui::TextEdit::singleline(&mut m.password).password(true).desired_width(240.0));
                    ui.end_row();
                }
            });
            if s.draft.backend == Backend::MySql {
                ui.label(
                    RichText::new(
                        "The database must already exist; tables and default categories are created \
                         automatically. The password is stored in plain text in config.json.",
                    )
                    .small()
                    .weak(),
                );
            }
        });

        ui.add_space(10.0);
        egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
            ui.label(RichText::new("Region").strong());
            ui.add_space(4.0);
            let system = Locale::system_tag();
            let draft = &mut self.settings.draft;
            egui::Grid::new("region").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                ui.label("Locale");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.locale)
                            .hint_text(format!("Automatic ({system})"))
                            .desired_width(160.0),
                    );
                    if !draft.locale.is_empty() && ui.small_button("Automatic").clicked() {
                        draft.locale.clear();
                    }
                });
                ui.end_row();
                let preview = draft.resolve_locale();
                ui.label("Currency symbol");
                ui.horizontal(|ui| {
                    let without_override = Config { currency_override: String::new(), ..draft.clone() };
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.currency_override)
                            .hint_text(without_override.resolve_locale().currency_symbol)
                            .desired_width(60.0),
                    );
                    ui.label(RichText::new("leave blank to use the locale's currency").small().weak());
                });
                ui.end_row();
                ui.label("Preview");
                ui.label(
                    RichText::new(format!(
                        "{}  ·  {}  ·  {}",
                        preview.format_money(123456),
                        preview.format_date(super::today()),
                        preview.month_label(super::month_key(super::today()))
                    ))
                    .strong(),
                );
                ui.end_row();
            });
            ui.label(
                RichText::new("Locale tags look like en-GB, en-US, de-DE or fr-FR. Leave blank to follow the system.")
                    .small()
                    .weak(),
            );
        });

        ui.add_space(10.0);
        egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
            ui.label(RichText::new("Updates").strong());
            ui.add_space(4.0);
            ui.checkbox(&mut self.settings.draft.check_for_updates, "Check for a new version at startup");
            ui.horizontal(|ui| {
                ui.label(format!("This is version {}.", env!("CARGO_PKG_VERSION")));
                let checking = self.update_check.is_some();
                if ui.add_enabled(!checking, egui::Button::new("Check now")).clicked() {
                    self.start_update_check(ui.ctx());
                }
                if checking {
                    ui.spinner();
                }
            });
            match &self.update_outcome {
                Some(update::Outcome::UpToDate) => {
                    ui.label("✔ You have the latest version.");
                }
                Some(update::Outcome::Available(release)) => {
                    ui.hyperlink_to(
                        format!("⬆ Version {} is available — download it from GitHub", release.version),
                        &release.url,
                    );
                }
                Some(update::Outcome::Failed(e)) => {
                    ui.label(
                        RichText::new(format!("✖ Could not check for updates: {e}")).color(ui.visuals().error_fg_color),
                    );
                }
                None => {}
            }
            ui.label(
                RichText::new(
                    "Checking asks GitHub for the latest release. Nothing is downloaded or installed automatically.",
                )
                .small()
                .weak(),
            );
        });

        ui.add_space(10.0);
        egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
            ui.label(RichText::new("Keyboard shortcuts").strong());
            ui.add_space(4.0);
            egui::Grid::new("shortcuts").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                for (keys, action) in super::shortcuts::describe(ui.ctx()) {
                    ui.label(RichText::new(keys).monospace());
                    ui.label(action);
                    ui.end_row();
                }
            });
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button("🔌 Test connection").clicked() {
                self.test_connection();
            }
            let save = ui.ctx().format_shortcut(&super::shortcuts::SAVE);
            if ui.add(egui::Button::new("💾 Save & connect").shortcut_text(save)).clicked() {
                self.apply_settings();
            }
            if ui.button("Revert").clicked() {
                self.settings = State::new(&self.cfg);
            }
        });
        if let Some((ok, msg)) = &self.settings.test_result {
            let colour = if *ok { egui::Color32::from_rgb(60, 170, 90) } else { ui.visuals().error_fg_color };
            ui.label(RichText::new(msg).color(colour));
        }

        ui.add_space(16.0);
        ui.separator();
        ui.label(RichText::new("Files").strong());
        for (label, path) in [
            ("Data folder", config::data_dir()),
            ("Settings", config::config_path()),
            ("Debug log", config::log_path()),
        ] {
            let text = path.display().to_string();
            ui.horizontal(|ui| {
                ui.add_sized([90.0, 18.0], egui::Label::new(label));
                if ui.small_button("📋").on_hover_text("Copy path").clicked() {
                    ui.ctx().copy_text(text.clone());
                }
                ui.add(egui::Label::new(RichText::new(&text).monospace()).truncate()).on_hover_text(&text);
            });
        }
    }

    fn test_connection(&mut self) {
        // Only looks: testing never creates a file, table or category.
        let result = self.settings.build().and_then(|cfg| db::test(&cfg).map_err(|e| format!("{e:#}")));
        self.settings.test_result = Some(match result {
            Ok(found) => {
                log::info!("Connection test succeeded: {found}");
                (true, format!("✔ {found}"))
            }
            Err(e) => {
                log::warn!("Connection test failed: {e}");
                (false, format!("✖ {e}"))
            }
        });
    }

    pub(super) fn apply_settings(&mut self) {
        let cfg = match self.settings.build() {
            Ok(c) => c,
            Err(e) => {
                self.settings.test_result = Some((false, format!("✖ {e}")));
                return;
            }
        };
        let db_changed = cfg.backend != self.cfg.backend
            || cfg.sqlite_path != self.cfg.sqlite_path
            || cfg.mysql != self.cfg.mysql
            || self.store.is_none();
        if db_changed {
            match db::open(&cfg) {
                Ok(store) => {
                    self.store = Some(store);
                    self.tx = Default::default();
                    self.cats = Default::default();
                }
                Err(e) => {
                    log::error!("Could not switch database: {e:#}");
                    self.settings.test_result =
                        Some((false, format!("✖ {e:#} — settings not saved, still using the previous database")));
                    return;
                }
            }
        }
        self.cfg = cfg;
        self.locale = self.cfg.resolve_locale();
        log::info!("Using locale {} (currency '{}')", self.locale.tag, self.locale.currency_symbol);
        if let Err(e) = self.cfg.save() {
            log::error!("Saving config failed: {e:#}");
            self.set_error(format!("Could not save settings: {e:#}"));
        } else {
            self.set_info("Settings saved".into());
        }
        self.settings = State::new(&self.cfg);
        self.settings.test_result = Some((true, "✔ Settings saved".into()));
        self.reload();
    }
}
