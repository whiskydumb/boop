use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

const REPO: &str = "whiskydumb/boop";
const USER_AGENT: &str = concat!("boop/", env!("CARGO_PKG_VERSION"));
const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
}

/// @note blocking and network-bound - run it off the UI thread. failures
/// (offline, rate-limited, unparseable) are real errors so the caller can log
/// them, but they are non-fatal: a failed check just means no notice is shown.
/// @return the newer release, or `None` when already up to date
pub fn check() -> Result<Option<UpdateInfo>> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let release: Release = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .context("failed to build http client")?
        .get(&url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .context("failed to reach github")?
        .error_for_status()
        .context("github returned an error status")?
        .json()
        .context("failed to parse the github release response")?;

    let latest = semver::Version::parse(release.tag_name.trim_start_matches('v'))
        .with_context(|| format!("unparseable release tag '{}'", release.tag_name))?;
    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .expect("crate version is valid semver");

    if latest > current {
        Ok(Some(UpdateInfo {
            version: latest.to_string(),
            url: release.html_url,
        }))
    } else {
        Ok(None)
    }
}
