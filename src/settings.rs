//! User settings, stored as JSON in the app's config folder. The budget
//! itself is in the database; see `db.rs`.

use crate::money::YearStart;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Shown with amounts, such as "£". May be empty. `None` is the
    /// region's usual symbol.
    pub currency: Option<String>,
    /// How dates and amounts are written, as a tag from `locale::REGIONS`.
    /// `None` is the system's region.
    pub region: Option<String>,
    /// The interface's language, as a tag from `i18n::LANGUAGES`. `None` is
    /// the system's language.
    pub language: Option<String>,
    /// When the financial year starts. `None` is the region's usual start.
    pub year_start: Option<YearStart>,
    /// Write a debug log.
    pub debug_log: bool,
    /// Where the debug log goes. `None` is the default folder.
    pub log_dir: Option<PathBuf>,
    /// Whether keyboard shortcuts work. Moving between tabs with the arrow
    /// keys, and Escape keeping focus where it is, always work.
    pub shortcuts: bool,
    /// Ask before deleting an entry.
    pub confirm_delete: bool,
    /// Ask GitHub for a newer release when the app starts.
    pub check_updates: bool,
    /// Keep the budget on `server` rather than in the built-in database.
    pub use_server: bool,
    /// A MySQL or MariaDB server, kept even while the built-in database is
    /// used, so its details needn't be typed again.
    pub server: Server,
}

/// How to reach a MySQL or MariaDB server.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    /// Kept in the settings file, which only the user can read.
    pub password: String,
    /// Encrypt the connection, checking the server's certificate.
    pub tls: bool,
}

impl Default for Server {
    fn default() -> Self {
        Self { host: String::new(), port: 3306, database: String::new(), user: String::new(), password: String::new(), tls: true }
    }
}

impl Server {
    /// Where the budget is, as shown, such as "budget on db.example.com:3306".
    /// Never the password.
    pub fn describe(&self) -> String {
        t!("server-describe", database = &self.database, host = self.host.trim(), port = self.port.to_string())
    }
}

/// Leaves the password out, so it can't reach the log by accident.
impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("user", &self.user)
            .field("password", &if self.password.is_empty() { "" } else { "(hidden)" })
            .field("tls", &self.tls)
            .finish()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            currency: None,
            region: None,
            language: None,
            year_start: None,
            debug_log: false, log_dir: None, shortcuts: true, confirm_delete: true,
            check_updates: true,
            use_server: false,
            server: Server::default(),
        }
    }
}

/// The settings file was there but couldn't be read, so the defaults are
/// in use. Said once the interface's language is known.
#[derive(Debug, PartialEq, Eq)]
pub enum Unreadable {
    /// A copy of it is here, safe from being saved over.
    Kept(PathBuf),
    /// No copy could be made.
    Lost,
}

impl Unreadable {
    /// What to tell the user.
    pub fn message(&self) -> String {
        match self {
            Unreadable::Kept(file) => t!("settings-unreadable-kept", file = file.display().to_string()),
            Unreadable::Lost => t!("settings-unreadable"),
        }
    }
}

impl Settings {
    pub fn load() -> (Self, Option<Unreadable>) {
        Self::load_from(&crate::paths::settings_file())
    }

    /// Reads the settings from `path`. Without a file, the defaults are
    /// used, as on the first run. A file that can't be read, such as one
    /// cut short or edited by hand, is copied aside first, as the next
    /// save would write over it.
    fn load_from(path: &Path) -> (Self, Option<Unreadable>) {
        let error = match std::fs::read_to_string(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Settings::default(), None),
            Ok(text) => match serde_json::from_str(&text) {
                Ok(settings) => return (settings, None),
                Err(e) => e.to_string(),
            },
            Err(e) => e.to_string(),
        };
        log::warn!("could not read the settings, so using the defaults: {error}");
        let kept = path.with_extension("json.unreadable");
        let problem = match std::fs::copy(path, &kept) {
            Ok(_) => Unreadable::Kept(kept),
            Err(e) => {
                log::warn!("could not keep a copy of the settings: {e}");
                Unreadable::Lost
            }
        };
        (Settings::default(), Some(problem))
    }

    pub fn save(&self) {
        let path = crate::paths::settings_file();
        let result = crate::paths::create_private_dir(&crate::paths::config_dir())
            .and_then(|()| crate::paths::write_file(&path, &serde_json::to_vec_pretty(self).unwrap_or_default()));
        if let Err(e) = result {
            log::warn!("could not save the settings: {e}");
        }
    }

    pub fn log_dir(&self) -> PathBuf {
        self.log_dir.clone().unwrap_or_else(crate::paths::default_log_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unreadable_file_is_kept_and_the_defaults_used() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(Settings::load_from(&path).1, None, "no file is the first run");

        std::fs::write(&path, r#"{"currency": "€", "debug_log": tr"#).unwrap();
        let (settings, problem) = Settings::load_from(&path);
        assert_eq!(settings.currency, None, "the defaults");
        let kept = dir.path().join("settings.json.unreadable");
        assert_eq!(problem, Some(Unreadable::Kept(kept.clone())));
        assert_eq!(std::fs::read_to_string(kept).unwrap(), r#"{"currency": "€", "debug_log": tr"#);

        std::fs::write(&path, r#"{"currency": "€"}"#).unwrap();
        let (settings, problem) = Settings::load_from(&path);
        assert_eq!((settings.currency.as_deref(), problem), (Some("€"), None));
    }
}
