use std::path::{Path, PathBuf};

use egui::RichText;

use super::App;
use crate::backup::{self, Backup};
use crate::config::{self, Appearance, Backend, Config};
use crate::db;
use crate::locale::Locale;
use crate::update;

pub struct State {
    draft: Config,
    sqlite_path: String,
    port: String,
    test_result: Option<(bool, String)>,
    /// A backup read from a file, waiting for the user to confirm the restore.
    restore: Option<(PathBuf, Backup)>,
    /// This computer's identity, used when none is chosen, or why it is unknown.
    automatic_identity: Result<String, String>,
}

impl State {
    pub fn new(cfg: &Config) -> Self {
        Self {
            draft: cfg.clone(),
            sqlite_path: cfg.sqlite_path.display().to_string(),
            port: cfg.mysql.port.to_string(),
            test_result: None,
            restore: None,
            automatic_identity: crate::identity::automatic().map_err(|e| format!("{e:#}")),
        }
    }

    /// Whether the restore confirmation is open.
    pub(super) fn dialog_open(&self) -> bool {
        self.restore.is_some()
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
            cfg.mysql.identity = cfg.mysql.identity.trim().to_string();
            if !cfg.mysql.identity.is_empty() {
                crate::identity::check(&cfg.mysql.identity).map_err(|e| e.to_string())?;
            } else if let Err(e) = &self.automatic_identity {
                return Err(format!("Enter an identity: {e}"));
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
                    ui.label("Identity");
                    let automatic = match &s.automatic_identity {
                        Ok(id) => id.as_str(),
                        Err(_) => "Required: this computer's couldn't be found",
                    };
                    ui.add(
                        egui::TextEdit::singleline(&mut m.identity)
                            .hint_text(automatic)
                            .char_limit(crate::identity::MAX_CHARS)
                            .desired_width(240.0),
                    )
                    .on_hover_text(
                        "Who you are on a shared database: others can't see your transactions. Filled in with \
                         this computer's identity the first time you connect, and kept if the computer is \
                         renamed. Enter the same identity on each computer you use to see your transactions on \
                         all of them, or clear it to use this computer's again.",
                    );
                    ui.end_row();
                }
            });
            if s.draft.backend == Backend::MySql {
                ui.label(
                    RichText::new(
                        "The database must already exist; tables and default categories are created \
                         automatically. The password is stored in plain text in config.json. Identity is \
                         filled in when you connect; enter the same one on each computer you use.",
                    )
                    .small()
                    .weak(),
                );
            }
            if let Some(me) = self.store.as_ref().and_then(|st| st.me()).map(str::to_owned) {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("You are");
                    ui.label(RichText::new(&me).monospace())
                        .on_hover_text("Your identity on this shared database. Give it to the database's administrator to be added to a team.");
                    if ui.small_button("📋").on_hover_text("Copy").clicked() {
                        ui.ctx().copy_text(me.clone());
                    }
                });
                let teams = if self.teams.is_empty() {
                    "You aren't in a team, so only you can see your transactions.".to_string()
                } else {
                    let names: Vec<&str> = self.teams.iter().map(|t| t.name.as_str()).collect();
                    format!("Your teams: {}. Transactions you share with a team are visible to its members.", names.join(", "))
                };
                ui.label(
                    RichText::new(format!(
                        "Other people using this database can't see your transactions. {teams} Categories are \
                         shared by everyone."
                    ))
                    .small()
                    .weak(),
                );
            }
        });

        ui.add_space(10.0);
        self.appearance_section(ui);

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

        ui.add_space(10.0);
        self.backup_section(ui);

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
        self.confirm_restore_dialog(ui);
    }

    /// Takes effect and is saved straight away, without the rest of the form.
    fn appearance_section(&mut self, ui: &mut egui::Ui) {
        let mut chosen = self.cfg.appearance;
        egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
            ui.label(RichText::new("Appearance").strong());
            ui.add_space(4.0);
            for a in Appearance::ALL {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut chosen, a, a.label());
                    ui.label(RichText::new(a.description()).weak());
                });
            }
        });
        if chosen == self.cfg.appearance {
            return;
        }
        super::appearance::apply(ui.ctx(), chosen);
        self.cfg.appearance = chosen;
        self.settings.draft.appearance = chosen;
        match self.cfg.save() {
            Ok(()) => self.set_info(format!("Appearance: {}", chosen.label())),
            Err(e) => {
                log::error!("Saving config failed: {e:#}");
                self.set_error(format!("Could not save settings: {e:#}"));
            }
        }
    }

    fn backup_section(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
            ui.label(RichText::new("Backup").strong());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("💾 Back up…").on_hover_text("Save every category and your transactions to a file").clicked() {
                    self.back_up();
                }
                if ui.button("♻ Restore…").on_hover_text("Replace everything with the contents of a backup").clicked()
                {
                    self.choose_restore_file();
                }
            });
            ui.label(
                RichText::new(
                    "A backup is one file holding every category and your transactions. It restores into SQLite \
                     or MariaDB/MySQL, so it also moves your data between them. Before a restore replaces your \
                     data, a copy of it is saved in the data folder's backups folder. On a shared MariaDB/MySQL \
                     database only your own transactions are backed up and replaced.",
                )
                .small()
                .weak(),
            );
        });
    }

    fn back_up(&mut self) {
        let Some(store) = self.store.as_mut() else {
            return self.set_error("No database is connected — check Settings".into());
        };
        let backup = match Backup::of(store.as_mut()) {
            Ok(b) => b,
            Err(e) => return self.set_error(format!("Could not back up: {e:#}")),
        };
        let Some(path) = rfd::FileDialog::new()
            .set_title("Back up")
            .set_file_name(backup::suggested_name())
            .add_filter("Backup", &["json"])
            .save_file()
        else {
            return;
        };
        match backup.write(&path) {
            Ok(()) => self.set_info(format!(
                "Backed up {} categories and {} transactions to {}",
                backup.categories.len(),
                backup.spending.len(),
                path.display()
            )),
            Err(e) => {
                log::error!("Backup failed: {e:#}");
                self.set_error(format!("Could not back up: {e:#}"));
            }
        }
    }

    fn choose_restore_file(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("Restore a backup")
            .add_filter("Backup", &["json"])
            .add_filter("All files", &["*"])
            .pick_file();
        if let Some(path) = picked {
            self.open_restore(&path);
        }
    }

    /// Read a backup and ask whether to restore it.
    pub(super) fn open_restore(&mut self, path: &Path) {
        if self.store.is_none() {
            return self.set_error("No database is connected — check Settings".into());
        }
        match Backup::read(path) {
            Ok(b) => self.settings.restore = Some((path.to_path_buf(), b)),
            Err(e) => {
                log::error!("Restore failed: {e:#}");
                self.set_error(format!("Could not restore: {e:#}"));
            }
        }
    }

    fn confirm_restore_dialog(&mut self, ui: &mut egui::Ui) {
        let Some((path, b)) = &self.settings.restore else { return };
        let mut close = false;
        let mut confirmed = false;
        let made = b.created.map_or("an unknown date".into(), |t| {
            let local = t.to_zoned(jiff::tz::TimeZone::system());
            format!("{} at {}", self.locale.format_date(local.date()), local.strftime("%H:%M"))
        });
        let target = self.store.as_ref().map_or_else(String::new, |s| s.describe());
        let shared = self.shared;
        let own = if shared {
            self.spending.iter().filter(|s| s.owner.as_deref() == self.me()).count()
        } else {
            self.spending.len()
        };
        let response = egui::Modal::new(egui::Id::unique("confirm_restore")).show(ui.ctx(), |ui| {
            ui.set_max_width(520.0);
            ui.heading("Restore this backup?");
            ui.add_space(6.0);
            let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into());
            ui.label(format!(
                "{name} was made on {made}. It has {} categories and {} transactions.",
                b.categories.len(),
                b.spending.len()
            ));
            if !b.renamed.is_empty() {
                let list: Vec<String> = b.renamed.iter().map(|(old, new)| format!("'{old}' as '{new}'")).collect();
                ui.label(format!(
                    "Some category names differ only by case, which is no longer allowed, so these are restored \
                     under new names: {}.",
                    list.join(", ")
                ));
            }
            ui.add_space(4.0);
            let replaces = if shared {
                format!(
                    "It replaces your {own} transactions in this shared database, as private transactions. \
                     Its categories are added if missing; other people's transactions and the other categories \
                     are kept."
                )
            } else {
                format!(
                    "It replaces everything in the current database: {} categories and {own} transactions.",
                    self.categories.len()
                )
            };
            ui.label(RichText::new(replaces).color(ui.visuals().warn_fg_color));
            ui.add(egui::Label::new(RichText::new(&target).small().weak()).truncate()).on_hover_text(&target);
            ui.label(RichText::new("A copy of them is saved first, in the data folder's backups folder.").small().weak());
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("♻ Replace and restore").clicked() {
                    confirmed = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if confirmed {
            if let Some((_, b)) = self.settings.restore.take() {
                self.restore(&b);
            }
        } else if close || response.should_close() {
            self.settings.restore = None;
        }
    }

    fn restore(&mut self, b: &Backup) {
        // Keep a copy of what is about to be replaced. If that fails, stop.
        let copy = backup::safety_copy_path();
        let saved = self.store.as_mut().map(|s| Backup::of(s.as_mut()).and_then(|current| current.write(&copy)));
        if let Some(Err(e)) = saved {
            log::error!("Could not save a copy before restoring: {e:#}");
            return self.set_error(format!("Nothing was restored, because a copy of the current data couldn't be saved: {e:#}"));
        }
        let what = if self.shared {
            // Categories are only added if missing, and only the user's own
            // transactions were replaced.
            format!(
                "Restored {} of your transactions; categories the backup needed were added if missing. \
                 Your previous transactions were saved to {}",
                b.spending.len(),
                copy.display()
            )
        } else {
            format!(
                "Restored {} categories and {} transactions. The previous data was saved to {}",
                b.categories.len(),
                b.spending.len(),
                copy.display()
            )
        };
        if self.run(&what, |db| db.replace_all(&b.categories, &b.spending)) {
            // Ids in forms and filters may now mean something else.
            self.tx = Default::default();
            self.cats = Default::default();
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
        if db_changed {
            self.pin_identity();
        }
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
