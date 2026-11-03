//! Who is using the app, for keeping people's transactions apart on a shared
//! MariaDB/MySQL database.
//!
//! By default this is, on Windows, the signed-in user's security identifier
//! (SID), such as `S-1-5-21-…-1001`, which stays the same if the account is
//! renamed; on macOS and Linux it is `login@computer`, as login names alone
//! are often the same on different computers. Someone can choose their own
//! under Settings instead, for example to be the same person on each computer
//! they use. The app reports the identity itself, so it separates people who
//! use the app as intended; it is not a security boundary against someone
//! who has the database password, another client or someone else's identity.

use anyhow::{Context, Result};

/// Longest identity the `owner` and `member` columns hold.
pub const MAX_CHARS: usize = 190;

/// The identity to use: `chosen` (from Settings) unless it is blank, in
/// which case this computer's [`automatic`] one.
pub fn resolve(chosen: &str) -> Result<String> {
    let chosen = chosen.trim();
    if chosen.is_empty() { automatic() } else { check(chosen) }
}

/// This computer's identity for the signed-in user.
pub fn automatic() -> Result<String> {
    check(os_identity()?.trim())
}

/// `id`, if it can be used as an identity.
pub fn check(id: &str) -> Result<String> {
    let id = id.trim();
    anyhow::ensure!(!id.is_empty(), "couldn't tell which user is signed in; set your identity in Settings");
    anyhow::ensure!(id.chars().count() <= MAX_CHARS, "an identity must be at most {MAX_CHARS} characters");
    Ok(id.to_string())
}

#[cfg(windows)]
fn os_identity() -> Result<String> {
    use std::os::windows::process::CommandExt;
    /// Don't flash a console window from the windowed app.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // By full path, so a whoami.exe elsewhere on PATH isn't run instead.
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let whoami = std::path::Path::new(&root).join("System32").join("whoami.exe");
    let out = std::process::Command::new(whoami)
        .args(["/user", "/fo", "csv", "/nh"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .context("running whoami to find the signed-in user's SID")?;
    anyhow::ensure!(out.status.success(), "whoami couldn't find the signed-in user's SID");
    sid_from_whoami(&String::from_utf8_lossy(&out.stdout))
        .context("reading the signed-in user's SID from whoami")
}

#[cfg(not(windows))]
fn os_identity() -> Result<String> {
    Ok(login_at_host(&login_name()?, &host_name()?))
}

#[cfg(not(windows))]
fn login_name() -> Result<String> {
    for var in ["USER", "LOGNAME"] {
        if let Some(name) = std::env::var(var).ok().filter(|n| !n.trim().is_empty()) {
            return Ok(name);
        }
    }
    command_output("id", &["-un"]).context("finding the login name")
}

/// This computer's name. On macOS the local host name, which unlike
/// `hostname` doesn't change with the network it is on.
#[cfg(not(windows))]
fn host_name() -> Result<String> {
    if cfg!(target_os = "macos") {
        if let Ok(name) = command_output("scutil", &["--get", "LocalHostName"]) {
            return Ok(name);
        }
    } else if let Ok(name) = std::fs::read_to_string("/proc/sys/kernel/hostname")
        && !name.trim().is_empty()
    {
        return Ok(name);
    }
    command_output("uname", &["-n"]).context("finding this computer's name; set your identity in Settings")
}

#[cfg(not(windows))]
fn command_output(program: &str, args: &[&str]) -> Result<String> {
    let out = std::process::Command::new(program).args(args).output().with_context(|| format!("running {program}"))?;
    anyhow::ensure!(out.status.success(), "{program} failed");
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    anyhow::ensure!(!text.is_empty(), "{program} printed nothing");
    Ok(text)
}

/// `login@host`, with the host's domain left off and in lower case, since
/// host names ignore case: `Sams-MacBook.local` gives `sam@sams-macbook`.
#[cfg_attr(windows, allow(dead_code))]
fn login_at_host(login: &str, host: &str) -> String {
    let host = host.trim();
    let host = host.split('.').next().unwrap_or(host).to_lowercase();
    format!("{}@{host}", login.trim())
}

/// The SID from `whoami /user /fo csv /nh`, which prints `"domain\user","S-1-5-…"`.
#[cfg_attr(not(windows), allow(dead_code))]
fn sid_from_whoami(output: &str) -> Result<String> {
    let line = output.lines().find(|l| !l.trim().is_empty()).context("whoami printed nothing")?;
    let sid = line.rsplit(',').next().unwrap_or_default().trim().trim_matches('"');
    anyhow::ensure!(sid.starts_with("S-1-"), "unexpected whoami output: {line}");
    Ok(sid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_sid_from_whoami() {
        let out = "\r\n\"desktop-1\\sam\",\"S-1-5-21-1004336348-1177238915-682003330-1001\"\r\n";
        assert_eq!(sid_from_whoami(out).unwrap(), "S-1-5-21-1004336348-1177238915-682003330-1001");
        assert!(sid_from_whoami("").is_err());
        assert!(sid_from_whoami("\"x\",\"not a sid\"").is_err());
    }

    #[test]
    fn names_the_computer() {
        assert_eq!(login_at_host("sam", "Sams-MacBook.local\n"), "sam@sams-macbook");
        assert_eq!(login_at_host("sam", "box"), "sam@box");
    }

    #[test]
    fn chosen_identity_wins() {
        assert_eq!(resolve("  sam.jones  ").unwrap(), "sam.jones");
        assert_eq!(resolve("").unwrap(), automatic().unwrap());
        assert!(resolve(&"x".repeat(MAX_CHARS + 1)).is_err());
    }

    #[test]
    fn current_user_is_known() {
        let id = automatic().unwrap();
        assert!(!id.is_empty());
        #[cfg(not(windows))]
        assert!(id.contains('@'), "{id}");
    }
}
