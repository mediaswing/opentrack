//! Application settings, persisted as JSON in the app's data directory.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backend {
    Sqlite,
    MySql,
}

/// Colour scheme and font weight.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    /// Light colours, Ubuntu Medium.
    #[default]
    Standard,
    /// White and yellow on black, Ubuntu Bold.
    HighContrast,
}

impl Appearance {
    pub const ALL: [Appearance; 2] = [Appearance::Standard, Appearance::HighContrast];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MySqlSettings {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    /// Who the user is on a shared database ([`crate::identity`]); empty
    /// means this computer's automatic identity.
    pub identity: String,
}

impl Default for MySqlSettings {
    fn default() -> Self {
        Self {
            host: "localhost".into(),
            port: 3306,
            user: String::new(),
            password: String::new(),
            database: "opentrack".into(),
            identity: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub backend: Backend,
    pub sqlite_path: PathBuf,
    pub mysql: MySqlSettings,
    /// Locale tag such as `en-GB`; empty means "detect from the system".
    pub locale: String,
    /// Currency symbol override; empty means "use the locale's currency".
    pub currency_override: String,
    /// Ask GitHub for a newer release at startup.
    pub check_for_updates: bool,
    pub appearance: Appearance,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            backend: Backend::Sqlite,
            sqlite_path: data_dir().join("opentrack.db"),
            mysql: MySqlSettings::default(),
            locale: String::new(),
            currency_override: String::new(),
            check_for_updates: true,
            appearance: Appearance::Standard,
        }
    }
}

/// Directory holding the config file, the default SQLite database and `output.log`.
///
/// Can be overridden with the `OPENTRACK_HOME` environment variable.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("OPENTRACK_HOME") {
        return PathBuf::from(dir);
    }
    directories::ProjectDirs::from("", "", "opentrack")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn log_path() -> PathBuf {
    data_dir().join("output.log")
}

impl Config {
    /// The effective locale: the configured one, or the system's, plus any currency override.
    pub fn resolve_locale(&self) -> crate::locale::Locale {
        let tag = if self.locale.trim().is_empty() {
            crate::locale::Locale::system_tag()
        } else {
            self.locale.trim().to_string()
        };
        let mut locale = crate::locale::Locale::from_tag(&tag);
        let symbol = self.currency_override.trim();
        if !symbol.is_empty() {
            locale.currency_symbol = symbol.to_string();
        }
        locale
    }

    /// Load the config file. When it exists but can't be read, the defaults
    /// are used and the second value says so, for showing to the user. The
    /// bad file is first copied aside so that saving doesn't lose it.
    pub fn load() -> (Self, Option<String>) {
        load_from(&config_path())
    }

    pub fn save(&self) -> Result<()> {
        write_json(&config_path(), self)
    }
}

/// Where an unreadable config file is copied before the defaults replace it.
fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.invalid")
}

fn load_from(path: &Path) -> (Config, Option<String>) {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::info!("No config at {}; using defaults", path.display());
            return (Config::default(), None);
        }
        Err(e) => return invalid(path, &format!("it could not be read ({e})"), None),
    };
    match serde_json::from_str(&text) {
        Ok(cfg) => {
            log::info!("Loaded config from {}", path.display());
            (cfg, None)
        }
        Err(e) => invalid(path, &format!("it is not valid ({e})"), Some(&text)),
    }
}

fn invalid(path: &Path, why: &str, text: Option<&str>) -> (Config, Option<String>) {
    let backup = backup_path(path);
    let kept = match text {
        Some(text) => std::fs::write(&backup, text),
        None => std::fs::copy(path, &backup).map(|_| ()),
    };
    let message = match kept {
        Ok(()) => format!(
            "Settings in {} were not used because {why}; using defaults. The file was copied to {}.",
            path.display(),
            backup.display()
        ),
        Err(e) => format!(
            "Settings in {} were not used because {why}; using defaults. It could not be copied aside ({e}), \
             so check it before saving settings.",
            path.display()
        ),
    };
    log::warn!("{message}");
    (Config::default(), Some(message))
}

/// Writes the file in one step (a new file renamed over the old one), so a
/// crash or full disk mid-write can't leave half a config behind.
fn write_json(path: &Path, cfg: &Config) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(cfg)?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, text).with_context(|| format!("writing {}", temp.display()))?;
    std::fs::rename(&temp, path).with_context(|| format!("replacing {}", path.display()))?;
    log::info!("Saved config to {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_config_is_kept_aside() {
        let dir = std::env::temp_dir().join(format!("opentrack-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");

        assert_eq!(load_from(&path), (Config::default(), None), "a missing file is not an error");

        std::fs::write(&path, "{ \"backend\": \"MySql\", oops").unwrap();
        let (cfg, warning) = load_from(&path);
        assert_eq!(cfg, Config::default());
        assert!(warning.unwrap().contains("not valid"));
        assert_eq!(std::fs::read_to_string(backup_path(&path)).unwrap(), "{ \"backend\": \"MySql\", oops");

        let cfg = Config { locale: "de-DE".into(), ..Config::default() };
        write_json(&path, &cfg).unwrap();
        assert_eq!(load_from(&path), (cfg, None));
        assert!(!path.with_extension("json.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
