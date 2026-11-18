// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod db;
mod locale;
mod logger;
mod money;
#[cfg(any(target_os = "macos", windows))]
mod package;
mod ui;
mod update;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The app's name, as the window, the README and the installers give it.
pub const APP_TITLE: &str = "Open Budget Tracking";

/// The executable's name, as installed and typed: `opentrack`, from the
/// `[[bin]]` in Cargo.toml. It is also this crate's name, so log targets start with it.
pub const BIN_NAME: &str = env!("CARGO_CRATE_NAME");

const USAGE: &str = "Open Budget Tracking

Usage:
  opentrack                     Start the application
  opentrack --db PATH           Start it with the database at PATH
  opentrack --locale NAME       Start it showing dates and money for NAME, such as en_GB
  opentrack --log PATH          Start it writing the debug log to PATH
  opentrack --version           Print the version
  opentrack --help              Print this help
  opentrack --package-macos DIR Build an ad hoc signed installer package (.pkg) in DIR (macOS)
  opentrack --package-msi DIR   Build an .msi installer in DIR (Windows; needs the WiX Toolset)";

/// Release builds on Windows are GUI programs with no console, so `--version`
/// and `--help` would print nothing. Attach to the console of the command prompt
/// that started us, if there is one, so their output shows up.
#[cfg(all(windows, not(debug_assertions)))]
fn attach_parent_console() {
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    // SAFETY: AttachConsole takes a process id and has no other preconditions;
    // failure (no parent console, or one already attached) is harmless.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[derive(Default)]
struct Args {
    db: Option<PathBuf>,
    locale: Option<String>,
    log: Option<PathBuf>,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |what: &str| it.next().cloned().ok_or(format!("{arg} needs {what}."));
        match arg.as_str() {
            "--db" => parsed.db = Some(value("a file path")?.into()),
            "--locale" => parsed.locale = Some(value("a locale name, such as en_GB")?),
            "--log" => parsed.log = Some(value("a file path")?.into()),
            other => return Err(format!("Unknown option: {other}\n\n{USAGE}")),
        }
    }
    Ok(parsed)
}

/// The database's usual place: `OPENTRACK_DB`, or the user's data folder.
fn default_db_path() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os("OPENTRACK_DB") {
        return Ok(PathBuf::from(p));
    }
    let dir = dirs::data_dir()
        .ok_or("Your user data folder could not be found, so nothing you add will be kept.")?
        .join(BIN_NAME);
    std::fs::create_dir_all(&dir).map_err(|e| format!("The folder {} could not be made: {e}.", dir.display()))?;
    Ok(dir.join("opentrack.db"))
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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(all(windows, not(debug_assertions)))]
    if !args.is_empty() {
        attach_parent_console();
    }
    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            println!("{APP_TITLE} {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help" | "-h") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        #[cfg(target_os = "macos")]
        Some("--package-macos") => package(args.get(1)),
        #[cfg(windows)]
        Some("--package-msi") => package(args.get(1)),
        _ => match parse_args(&args) {
            Ok(args) => run_gui(args),
            Err(msg) => {
                eprintln!("{msg}");
                ExitCode::from(2)
            }
        },
    }
}

fn run_gui(args: Args) -> ExitCode {
    // If the database or the log can't be opened, the app still runs and the
    // status bar says what went wrong.
    let mut problems = Vec::new();
    let db_path = match args.db.map_or_else(default_db_path, Ok) {
        Ok(path) => Some(path),
        Err(msg) => {
            problems.push(msg);
            None
        }
    };
    let log_path = args.log.or_else(|| db_path.as_ref().map(|p| p.parent().unwrap_or(Path::new("")).join("debug.log")));
    if let Some(path) = &log_path
        && let Err(msg) = logger::init(path)
    {
        problems.push(format!("{msg} The app will run without one."));
    }
    log::info!("{APP_TITLE} {} starting on {} {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH);

    let opened = db_path.as_ref().map(|path| {
        log::info!("opening database {}", path.display());
        db::Db::open(path).map_err(|e| {
            format!("Your spending could not be opened from {}, so nothing you add will be kept: {e}.", path.display())
        })
    });
    let (db, database) = match opened {
        Some(Ok(db)) => (db, db_path),
        failed => {
            if let Some(Err(msg)) = failed {
                problems.push(msg);
            }
            match db::Db::open_in_memory() {
                Ok(db) => (db, None),
                Err(e) => {
                    eprintln!("Could not start: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    };

    // --locale wins, then the one chosen on the Settings tab, then the computer's.
    let wanted = args.locale.or_else(|| db.setting("locale").ok().flatten());
    let fmt = locale::LocaleFormat::detect(wanted.as_deref()).unwrap_or_else(|msg| {
        problems.push(msg);
        locale::LocaleFormat::detect(None).expect("detecting the computer's locale always succeeds")
    });
    log::info!("locale {}", fmt.describe());
    for problem in &problems {
        log::warn!("{problem}");
    }

    let paths = ui::Paths { database, log: log_path };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_app_id(BIN_NAME)
            .with_inner_size([900.0, 680.0])
            .with_min_inner_size([560.0, 420.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |cc| {
            let dialogs = Box::new(ui::NativeDialogs);
            let mut app = ui::App::new(&cc.egui_ctx, db, fmt, dialogs, paths, problems.join(" "));
            // Here rather than in App::new, so that tests never reach GitHub.
            app.check_for_updates_at_startup();
            Ok(Box::new(app))
        }),
    );
    let code = match result {
        Ok(()) => {
            log::info!("exited normally");
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error!("exited with error: {e}");
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    };
    log::logger().flush();
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_program_is_called_opentrack() {
        assert_eq!(BIN_NAME, "opentrack");
        assert!(USAGE.lines().skip(3).all(|line| line.trim_start().starts_with(BIN_NAME)), "{USAGE}");
    }

    #[test]
    fn options_are_read() {
        let args: Vec<String> = ["--db", "a.db", "--locale", "de_DE"].map(String::from).to_vec();
        let parsed = parse_args(&args).unwrap();
        assert_eq!(parsed.db.as_deref(), Some(Path::new("a.db")));
        assert_eq!(parsed.locale.as_deref(), Some("de_DE"));
        assert!(parse_args(&["--db".to_owned()]).is_err());
        assert!(parse_args(&["--nope".to_owned()]).is_err());
    }
}
