// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod config;
mod db;
mod locale;
mod money;
mod package;
mod ui;
mod update;

use std::fs::File;
use std::process::ExitCode;

use simplelog::{CombinedLogger, ConfigBuilder, LevelFilter, WriteLogger};

pub const APP_TITLE: &str = "Open Budget Tracker";

const USAGE: &str = "Open Budget Tracker

Usage:
  opentrack                     Start the application
  opentrack --version           Print the version
  opentrack --help              Print this help
  opentrack --package-macos DIR Build an installer package (.pkg) of the app, ad hoc signed, in DIR (macOS)
  opentrack --package-deb DIR   Build a .deb package in DIR (Linux)
  opentrack --package-msi DIR   Build an .msi installer in DIR (Windows; needs the WiX Toolset)";

/// Debug output from this app goes to `output.log`; other crates only log warnings and errors.
fn init_logging() {
    let path = config::log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = match File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Could not create log file {}: {e}", path.display());
            return;
        }
    };
    let ours = ConfigBuilder::new()
        .add_filter_allow_str(env!("CARGO_PKG_NAME"))
        .set_time_offset_to_local()
        .unwrap_or_else(|b| b)
        .build();
    let theirs = ConfigBuilder::new()
        .add_filter_ignore_str(env!("CARGO_PKG_NAME"))
        .set_time_offset_to_local()
        .unwrap_or_else(|b| b)
        .build();
    let mut loggers: Vec<Box<dyn simplelog::SharedLogger>> = Vec::new();
    if let Ok(second) = file.try_clone() {
        loggers.push(WriteLogger::new(LevelFilter::Warn, theirs, second));
    }
    loggers.push(WriteLogger::new(LevelFilter::Debug, ours, file));
    if CombinedLogger::init(loggers).is_err() {
        eprintln!("Logger already initialised");
    }

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("PANIC: {info}");
        default_hook(info);
    }));

    log::info!("{APP_TITLE} {} starting", env!("CARGO_PKG_VERSION"));
    log::info!("Logging to {}", path.display());
}

/// Release builds on Windows are GUI programs with no console, so `--version`
/// and `--help` would print nothing. Attach to the console of the command
/// prompt that started us, if there is one, so their output shows up. (When
/// the output is redirected to a file or pipe it already works, and this does
/// nothing.)
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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(all(windows, not(debug_assertions)))]
    if !args.is_empty() {
        attach_parent_console();
    }
    match args.first().map(String::as_str) {
        None => run_gui(),
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
        #[cfg(all(unix, not(target_os = "macos")))]
        Some("--package-deb") => package(args.get(1)),
        #[cfg(windows)]
        Some("--package-msi") => package(args.get(1)),
        Some(other) => {
            eprintln!("Unknown option: {other}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

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

fn run_gui() -> ExitCode {
    init_logging();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            // Matches StartupWMClass in the .desktop file (src/package/linux.rs).
            .with_app_id(env!("CARGO_PKG_NAME"))
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([760.0, 480.0]),
        ..Default::default()
    };
    let result = eframe::run_native(APP_TITLE, options, Box::new(|cc| Ok(Box::new(ui::App::new(cc)))));
    match result {
        Ok(()) => {
            log::info!("Exited normally");
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::error!("Exited with error: {e}");
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
