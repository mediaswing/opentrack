//! Checks GitHub for a newer release of the app.
//!
//! Only checks and reports: nothing is downloaded or installed. The request
//! runs on a background thread so the window never waits on the network.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;

/// The newest published release. GitHub leaves out drafts and pre-releases.
const LATEST_API: &str = "https://api.github.com/repos/mediaswing/opentrack/releases/latest";
const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// The release's page on GitHub.
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    UpToDate,
    Available(Release),
    Failed(String),
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

/// A check running in the background.
pub struct Check {
    rx: Receiver<Outcome>,
}

impl Check {
    /// Start a check. `ctx` is asked to repaint when it finishes.
    pub fn start(ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("update-check".into()).spawn(move || {
            let outcome = check(env!("CARGO_PKG_VERSION"));
            match &outcome {
                Outcome::UpToDate => log::info!("Update check: up to date"),
                Outcome::Available(r) => {
                    log::info!("Update check: {} is available ({})", r.version, r.url)
                }
                Outcome::Failed(e) => log::warn!("Update check failed: {e}"),
            }
            let _ = tx.send(outcome);
            ctx.request_repaint();
        });
        if let Err(e) = spawned {
            log::error!("Could not start the update check: {e}");
        }
        Self { rx }
    }

    /// The outcome, once the check has finished. Returns it only once.
    pub fn poll(&self) -> Option<Outcome> {
        match self.rx.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Outcome::Failed("the check stopped unexpectedly".into())),
        }
    }
}

fn check(current: &str) -> Outcome {
    match fetch_latest() {
        Ok(latest) => compare(current, latest),
        Err(e) => Outcome::Failed(format!("{e:#}")),
    }
}

fn fetch_latest() -> Result<GitHubRelease> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).build().into();
    let mut response = agent
        .get(LATEST_API)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|e| match e {
            // GitHub answers 404 until the first release is published.
            ureq::Error::StatusCode(404) => anyhow::anyhow!("no releases have been published yet"),
            e => anyhow::Error::new(e).context("contacting GitHub"),
        })?;
    let body = response.body_mut().read_to_string().context("reading GitHub's reply")?;
    serde_json::from_str(&body).context("reading GitHub's reply")
}

fn compare(current: &str, latest: GitHubRelease) -> Outcome {
    let parse = |v: &str| Version::parse(v.trim().trim_start_matches('v'));
    let current = match parse(current) {
        Ok(v) => v,
        Err(e) => return Outcome::Failed(format!("this version '{current}' is not valid: {e}")),
    };
    let version = match parse(&latest.tag_name) {
        Ok(v) => v,
        Err(e) => {
            return Outcome::Failed(format!("the latest release's tag '{}' is not valid: {e}", latest.tag_name));
        }
    };
    if version > current { Outcome::Available(Release { version, url: latest.html_url }) } else { Outcome::UpToDate }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> GitHubRelease {
        GitHubRelease { tag_name: tag.into(), html_url: format!("https://github.com/mediaswing/opentrack/releases/tag/{tag}") }
    }

    #[test]
    fn newer_release_is_available() {
        let Outcome::Available(r) = compare("0.1.0", release("v0.2.0")) else { panic!("expected an update") };
        assert_eq!(r.version, Version::new(0, 2, 0));
        assert!(r.url.ends_with("/tag/v0.2.0"));
    }

    #[test]
    fn same_or_older_is_up_to_date() {
        assert_eq!(compare("0.2.0", release("v0.2.0")), Outcome::UpToDate);
        assert_eq!(compare("0.10.0", release("v0.9.9")), Outcome::UpToDate);
    }

    #[test]
    fn release_is_newer_than_its_pre_release() {
        assert!(matches!(compare("1.0.0-beta.1", release("1.0.0")), Outcome::Available(_)));
    }

    #[test]
    fn bad_tag_fails() {
        assert!(matches!(compare("0.1.0", release("latest")), Outcome::Failed(_)));
    }
}
