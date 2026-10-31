//! Release packaging, which only the release workflow runs: an installer
//! package (.pkg) on macOS and a Debian package (.deb) on Linux. It is kept
//! apart from the code that runs when the app is used. Windows' installer is
//! built by WiX Toolset in the release workflow instead (see
//! src/installer/opentrack.wxs).

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
pub use linux::build;
#[cfg(target_os = "macos")]
pub use macos::build;
