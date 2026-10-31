//! Locations of the app's own files.

use std::path::{Path, PathBuf};

#[cfg(any(target_os = "windows", target_os = "macos"))]
const APP_DIR: &str = "OpenBudgetTracker";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const APP_DIR: &str = "opentrack";

/// Settings. On Windows this is `%APPDATA%\OpenBudgetTracker`.
pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join(APP_DIR)
}

pub fn settings_file() -> PathBuf {
    config_dir().join("settings.json")
}

/// The SQLite database. On Windows this is
/// `%LOCALAPPDATA%\OpenBudgetTracker\budget.db`.
pub fn database_file() -> PathBuf {
    data_dir().join("budget.db")
}

pub fn default_log_dir() -> PathBuf {
    data_dir().join("logs")
}

fn data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join(APP_DIR)
}

/// Creates a directory that only the current user can read, where the
/// platform supports it.
pub fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(dir)
    }
}

/// Writes a file atomically (write to a temporary file, then rename), so a
/// failure never leaves half a file.
pub fn write_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(contents)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Opens a web page in the default browser. Only `https://` addresses are
/// opened, so nothing else can be started this way.
pub fn open_url(url: &str) -> std::io::Result<()> {
    if !url.starts_with("https://") || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "refusing to open an unexpected address"));
    }
    open(url)
}

/// Opens a folder in File Explorer, Finder or the file manager.
pub fn open_folder(dir: &Path) -> std::io::Result<()> {
    open(dir)
}

fn open(target: impl AsRef<std::ffi::OsStr>) -> std::io::Result<()> {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program).arg(target).spawn().map(|_| ())
}
