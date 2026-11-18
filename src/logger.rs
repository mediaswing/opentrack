//! A minimal file logger that writes to `debug.log`.
//!
//! By default the log sits next to the database file. The level is `debug`
//! unless the `OPENTRACK_LOG` environment variable says otherwise (`off`,
//! `error`, `warn`, `info`, `debug` or `trace`). When the log grows past
//! 1 MB it is renamed to `debug.log.old` at start-up and a new one begun.
//!
//! The log records what the app does (commands, entry numbers, counts,
//! errors) but not what you bought or how much you spent.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use log::{LevelFilter, Log, Metadata, Record};

const MAX_BYTES: u64 = 1024 * 1024;

struct FileLogger {
    file: Mutex<File>,
    level: LevelFilter,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level && metadata.target().starts_with(env!("CARGO_CRATE_NAME"))
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(
                file,
                "{} {:<5} [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

fn level_from_env() -> LevelFilter {
    std::env::var("OPENTRACK_LOG")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(LevelFilter::Debug)
}

/// Start logging to `path`. Failure to open the log is not fatal: the app
/// still runs, and the caller is told why logging is unavailable.
pub fn init(path: &Path) -> Result<(), String> {
    let level = level_from_env();
    if level == LevelFilter::Off {
        return Ok(());
    }
    if fs::metadata(path).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = fs::rename(path, path.with_extension("log.old"));
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("Could not open the debug log at {}: {e}.", path.display()))?;
    log::set_boxed_logger(Box::new(FileLogger { file: Mutex::new(file), level }))
        .map_err(|e| format!("Could not start the debug log: {e}."))?;
    log::set_max_level(level);

    // Record panics in the log as well as on screen.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        log::logger().flush();
        default_hook(info);
    }));
    Ok(())
}
