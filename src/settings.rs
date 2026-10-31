//! User settings, stored as JSON in the app's config folder. The budget
//! itself is in the database; see `db.rs`.

use crate::money::YearStart;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        match std::fs::read_to_string(crate::paths::settings_file()) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                log::warn!("could not read the settings, so using the defaults: {e}");
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
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
