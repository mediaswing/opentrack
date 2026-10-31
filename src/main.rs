//! Open Budget Tracker (`opentrack`): an accessible, keyboard-first
//! budgeting tool with its own SQLite database.

// Release builds on Windows are GUI programs without a console window.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[macro_use]
mod i18n;

mod app;
mod db;
mod locale;
mod logging;
mod money;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod package;
mod paths;
mod settings;
mod shortcuts;
mod transfer;
mod update;

use std::process::ExitCode;

/// The app's name, as its windows, dialogs and files give it.
const APP_TITLE: &str = "Open Budget Tracker";

const USAGE: &str = "Open Budget Tracker

Usage:
  opentrack                     Start the application
  opentrack --debug-log         Start the application, writing the debug log
  opentrack --database FILE     Start the application with another database file
  opentrack --version           Print the version
  opentrack --help              Print this help
  opentrack --package-macos DIR Build an installer package (.pkg) of the app in DIR (macOS)
  opentrack --package-deb DIR   Build a Debian package (.deb) of the app in DIR (Linux)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => run_gui(false, None),
        Some("--debug-log") => run_gui(true, None),
        Some("--database") => match args.get(1) {
            Some(file) => run_gui(false, Some(std::path::PathBuf::from(file))),
            None => {
                eprintln!("--database needs a file

{USAGE}");
                ExitCode::from(2)
            }
        },
        Some("--version" | "-V") => {
            println!("opentrack {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help" | "-h") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        #[cfg(target_os = "macos")]
        Some("--package-macos") => package(args.get(1)),
        #[cfg(target_os = "linux")]
        Some("--package-deb") => package(args.get(1)),
        Some(other) => {
            eprintln!("Unknown option: {other}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn package(dir: Option<&String>) -> ExitCode {
    let dir = std::path::PathBuf::from(dir.map(String::as_str).unwrap_or("dist"));
    match package::build(&dir) {
        Ok(path) => {
            println!("{}", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Packaging failed: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_gui(force_log: bool, database: Option<std::path::PathBuf>) -> ExitCode {
    logging::init();
    let mut settings = settings::Settings::load();
    if force_log {
        settings.debug_log = true;
    }
    i18n::set(i18n::resolve(settings.language.as_deref()));
    let mut start_status = String::new();
    if settings.debug_log {
        match logging::start(&settings.log_dir()) {
            Ok(path) => start_status = t!("log-writing", path = path.display().to_string()),
            Err(e) => start_status = t!("log-off-error", error = e.to_string()),
        }
    }
    log::info!("starting opentrack {}", env!("CARGO_PKG_VERSION"));

    let db_path = database.unwrap_or_else(paths::database_file);
    let db = match db::Db::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            let message = t!("error-open-database", path = db_path.display().to_string(), error = format!("{e:#}"));
            log::error!("{message}");
            eprintln!("{message}");
            rfd::MessageDialog::new()
                .set_title(APP_TITLE)
                .set_description(message)
                .set_level(rfd::MessageLevel::Error)
                .show();
            return ExitCode::FAILURE;
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_app_id("opentrack")
            .with_inner_size([860.0, 860.0])
            .with_min_inner_size([480.0, 480.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon/opentrack.png"))
                    .expect("the bundled icon is a valid PNG"),
            ),
        ..Default::default()
    };
    let result = eframe::run_native(
        APP_TITLE,
        options,
        Box::new(move |cc| Ok(Box::new(app::BudgetApp::new(&cc.egui_ctx, db, db_path, settings, start_status)))),
    );
    log::info!("closing");
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("could not start the window: {e}");
            eprintln!("{}", t!("error-start-window", error = e.to_string()));
            ExitCode::FAILURE
        }
    }
}
