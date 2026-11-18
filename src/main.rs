mod db;
mod locale;
mod logger;
mod money;
#[cfg(any(target_os = "macos", windows))]
mod package;
mod ui;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The app's name, as the README, the app and the installers give it.
pub const APP_TITLE: &str = "Open Budget Tracking";

/// The executable's name, as installed and typed: `opentrack`, from the
/// `[[bin]]` in Cargo.toml. It is also this crate's name, so log targets start with it.
pub const BIN_NAME: &str = env!("CARGO_CRATE_NAME");

const USAGE: &str = "Open Budget Tracking

A simple budget tracker with a speech-friendly text interface.

Usage:
  opentrack [--db PATH] [--locale NAME] [--log PATH]

Options:
  --db PATH       Use the SQLite database at PATH.
  --locale NAME   Format dates and money for this locale, such as en_GB or de_DE.
  --log PATH      Write the debug log to PATH.
  --version       Print the version.
  --help          Show this message.

Packaging, used by the release workflow:
  --package-macos DIR   Build an ad hoc signed installer package (.pkg) in DIR (macOS)
  --package-msi DIR     Build an .msi installer in DIR (Windows; needs the WiX Toolset)

Database: without --db, the OPENTRACK_DB environment variable is used if set.
Otherwise the database is kept in your user data folder as
opentrack/opentrack.db, and is created on first run.

Locale: by default the system locale is detected. OPENTRACK_LOCALE overrides it.

Debug log: written to debug.log in the same folder as the database. Set
OPENTRACK_LOG to off, error, warn, info, debug or trace to change how much is
recorded. The default is debug.";

struct Args {
    db: Option<PathBuf>,
    locale: Option<String>,
    log: Option<PathBuf>,
    help: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args { db: None, locale: None, log: None, help: false };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |what: &str| it.next().ok_or(format!("{arg} needs {what}."));
        match arg.as_str() {
            "--help" | "-h" => args.help = true,
            "--db" => args.db = Some(value("a file path")?.into()),
            "--locale" => args.locale = Some(value("a locale name, such as en_GB")?),
            "--log" => args.log = Some(value("a file path")?.into()),
            other => return Err(format!("Unknown option {other}. Use --help to see the options.")),
        }
    }
    Ok(args)
}

fn default_db_path() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os("OPENTRACK_DB") {
        return Ok(PathBuf::from(p));
    }
    let dir = dirs::data_dir()
        .ok_or("Could not find your user data folder. Use --db to choose a database file.")?
        .join("opentrack");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}.", dir.display()))?;
    Ok(dir.join("opentrack.db"))
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let db_path = match args.db {
        Some(p) => p,
        None => default_db_path()?,
    };
    let log_path = args
        .log
        .unwrap_or_else(|| db_path.parent().unwrap_or(Path::new("")).join("debug.log"));
    if let Err(msg) = logger::init(&log_path) {
        eprintln!("{msg} Continuing without a debug log.");
    }
    log::info!(
        "{APP_TITLE} {} starting on {} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    let fmt = locale::LocaleFormat::detect(args.locale.as_deref())?;
    log::info!("locale {}", fmt.describe());

    if args.help {
        println!("{USAGE}\n\nCurrent locale: {}.\nDatabase: {}.\nDebug log: {}.",
            fmt.describe(), db_path.display(), log_path.display());
        return Ok(());
    }

    log::info!("opening database {}", db_path.display());
    let db = db::Db::open(&db_path).map_err(|e| format!("Could not open the database at {}: {e}.", db_path.display()))?;
    let stdin = std::io::stdin();
    ui::Ui::new(db, fmt, stdin.lock(), std::io::stdout().lock())
        .run()
        .map_err(|e| format!("Input or output failed: {e}."))
}

/// Builds an installer in `dir` (default `dist`) and prints its path.
#[cfg(any(target_os = "macos", windows))]
fn package(dir: Option<&String>) -> ExitCode {
    let dir = PathBuf::from(dir.map_or("dist", String::as_str));
    match package::build(&dir) {
        Ok(path) => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Could not build the installer: {e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Opened from Finder or the Dock, the app has no terminal to talk in, so ask
/// Terminal to run it in a new window instead, and stop. Returns false when the
/// app already has a terminal, or isn't running from an app bundle.
#[cfg(target_os = "macos")]
fn reopen_in_terminal() -> bool {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() || std::env::args().len() > 1 {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else { return false };
    if !exe.to_string_lossy().contains(".app/Contents/MacOS/") {
        return false;
    }
    std::process::Command::new("/usr/bin/open").args(["-a", "Terminal"]).arg(&exe).status().is_ok_and(|s| s.success())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            println!("{APP_TITLE} {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        #[cfg(target_os = "macos")]
        Some("--package-macos") => return package(args.get(1)),
        #[cfg(windows)]
        Some("--package-msi") => return package(args.get(1)),
        _ => {}
    }
    #[cfg(target_os = "macos")]
    if reopen_in_terminal() {
        return ExitCode::SUCCESS;
    }

    let result = run();
    if let Err(msg) = &result {
        log::error!("{msg}");
        eprintln!("{msg}");
    }
    log::logger().flush();
    if result.is_ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
