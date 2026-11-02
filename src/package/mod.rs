//! Release packaging, run with `opentrack --package-macos <dir>`,
//! `opentrack --package-deb <dir>` or `opentrack --package-msi <dir>`: an
//! installer package (.pkg) on macOS, a .deb on Linux and an .msi on Windows.
//! It packages the running executable, so build in release mode first. It is
//! kept apart from the code that runs when the app is used, and needs no shell
//! script.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::build;

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(all(unix, not(target_os = "macos")))]
pub use linux::build;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::build;
