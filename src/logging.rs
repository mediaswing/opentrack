//! The debug log, written only when it is turned on in Settings, to a folder
//! the user chooses.
//!
//! The log records what the app did (entry and category numbers, counts) and
//! any errors. It never records what money was spent on or how much, so it
//! is safe to attach to a bug report.
//!
//! Each session starts a fresh `opentrack.log`. The one before it is kept as
//! `opentrack.previous.log`, so a crash is still on disk after a restart.

use log::{LevelFilter, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const LOG_FILE: &str = "opentrack.log";
const PREVIOUS_LOG_FILE: &str = "opentrack.previous.log";

struct OpenLog {
    path: PathBuf,
    file: File,
}

struct FileLogger {
    log: Mutex<Option<OpenLog>>,
}

static LOGGER: FileLogger = FileLogger { log: Mutex::new(None) };

/// Every log file this session has started, so turning the log off and on
/// again carries on with it instead of moving it over the previous one.
static STARTED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // Only our own messages, not the chatter of every dependency.
        metadata.target().starts_with("opentrack") || metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let line = format!("{time} {:<5} {}: {}\n", record.level(), record.target(), record.args());
        if let Ok(mut guard) = self.log.lock()
            && let Some(open) = guard.as_mut()
        {
            let _ = open.file.write_all(line.as_bytes());
            let _ = open.file.flush();
        }
        if cfg!(debug_assertions) {
            eprint!("{line}");
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.log.lock()
            && let Some(open) = guard.as_mut()
        {
            let _ = open.file.flush();
        }
    }
}

pub fn init() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(LevelFilter::Debug);
}

/// Starts writing the log in `dir`, creating it if needed. Returns the log
/// file's path.
///
/// If this session is already writing to that folder, nothing changes. If
/// it wrote there earlier, the log carries on from where it stopped.
/// Otherwise any log already there belongs to an earlier session, so it is
/// moved aside to `opentrack.previous.log` and a fresh one is started.
pub fn start(dir: &Path) -> std::io::Result<PathBuf> {
    crate::paths::create_private_dir(dir)?;
    let path = dir.join(LOG_FILE);
    let mut guard = LOGGER.log.lock().unwrap_or_else(|e| e.into_inner());
    if guard.as_ref().is_some_and(|open| same_file(&open.path, &path)) {
        return Ok(path);
    }
    let mut started = STARTED.lock().unwrap_or_else(|e| e.into_inner());
    if started.iter().any(|p| same_file(p, &path))
        && std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file())
    {
        let file = OpenOptions::new().append(true).open(&path)?;
        *guard = Some(OpenLog { path: path.clone(), file });
        return Ok(path);
    }
    // Checked without following links, so a link left in the folder is moved
    // aside like a log would be.
    if std::fs::symlink_metadata(&path).is_ok() && std::fs::rename(&path, dir.join(PREVIOUS_LOG_FILE)).is_err() {
        let _ = std::fs::remove_file(&path);
    }
    // Only ever a new file: if something has appeared in its place since, a
    // link to another file perhaps, opening fails rather than overwriting it.
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&path)?;
    started.push(path.clone());
    *guard = Some(OpenLog { path: path.clone(), file });
    Ok(path)
}

/// Stops writing the log. What has been written stays on disk.
pub fn stop() {
    let mut guard = LOGGER.log.lock().unwrap_or_else(|e| e.into_inner());
    *guard = None;
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restarting_keeps_the_previous_log() {
        init();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LOG_FILE), "last session\n").unwrap();
        let path = start(dir.path()).unwrap();
        log::warn!("first");
        stop();
        start(dir.path()).unwrap();
        log::warn!("second");
        stop();
        let previous = std::fs::read_to_string(dir.path().join(PREVIOUS_LOG_FILE)).unwrap();
        assert_eq!(previous, "last session\n");
        let current = std::fs::read_to_string(path).unwrap();
        assert!(current.contains("first") && current.contains("second"), "{current}");
    }
}
