//! Who is using the app, for keeping people's transactions apart on a shared
//! MariaDB/MySQL database.
//!
//! On Windows this is the signed-in user's security identifier (SID), such
//! as `S-1-5-21-…-1001`, which stays the same if the account is renamed. On
//! macOS and Linux it is the login name. The app reports it itself, so it
//! separates people who use the app as intended; it is not a security
//! boundary against someone who has the database password and another client.

use anyhow::{Context, Result};

/// Longest identity the `owner` and `member` columns hold.
pub const MAX_CHARS: usize = 190;

/// The current user's identity.
pub fn current() -> Result<String> {
    let id = os_identity()?;
    let id = id.trim();
    anyhow::ensure!(!id.is_empty(), "couldn't tell which user is signed in");
    anyhow::ensure!(id.chars().count() <= MAX_CHARS, "the signed-in user's identity is too long: {id}");
    Ok(id.to_string())
}

#[cfg(windows)]
fn os_identity() -> Result<String> {
    use std::os::windows::process::CommandExt;
    /// Don't flash a console window from the windowed app.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("whoami")
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
    for var in ["USER", "LOGNAME"] {
        if let Some(name) = std::env::var(var).ok().filter(|n| !n.trim().is_empty()) {
            return Ok(name);
        }
    }
    let out = std::process::Command::new("id").arg("-un").output().context("running id to find the login name")?;
    anyhow::ensure!(out.status.success(), "id couldn't find the login name");
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
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
    fn current_user_is_known() {
        let id = current().unwrap();
        assert!(!id.is_empty());
    }
}
