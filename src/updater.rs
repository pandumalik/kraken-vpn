use std::time::Duration;

use serde::Deserialize;

pub const GITHUB_REPO: &str = "pandumalik/kraken-vpn";
pub const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/pandumalik/kraken-vpn/releases/latest";
pub const RELEASES_PAGE_URL: &str = "https://github.com/pandumalik/kraken-vpn/releases";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag_name: String,
    pub name: String,
    pub html_url: String,
    pub installer_url: Option<String>,
    pub published_at: Option<String>,
    pub notes_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheckResult {
    UpToDate {
        current: String,
        latest: String,
        release_url: String,
    },
    UpdateAvailable {
        current: String,
        release: ReleaseInfo,
    },
    NoReleases {
        current: String,
        releases_url: String,
    },
    Error(String),
}

#[derive(Debug, Deserialize)]
struct GithubReleasePayload {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GithubAssetPayload>,
}

#[derive(Debug, Deserialize)]
struct GithubAssetPayload {
    name: String,
    browser_download_url: String,
}

pub fn check_for_updates() -> UpdateCheckResult {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout_read(Duration::from_secs(10))
        .timeout_write(Duration::from_secs(6))
        .build();

    let user_agent = format!("kraken-vpn/{CURRENT_VERSION}");
    let response = agent
        .get(LATEST_RELEASE_API)
        .set("User-Agent", &user_agent)
        .set("Accept", "application/vnd.github+json")
        .call();

    match response {
        Ok(resp) => match resp.into_string() {
            Ok(body) => evaluate_release_json(CURRENT_VERSION, &body),
            Err(e) => UpdateCheckResult::Error(format!("Failed to read GitHub response: {e}")),
        },
        Err(ureq::Error::Status(404, _)) => UpdateCheckResult::NoReleases {
            current: CURRENT_VERSION.to_string(),
            releases_url: RELEASES_PAGE_URL.to_string(),
        },
        Err(ureq::Error::Status(403, _)) => UpdateCheckResult::Error(
            "GitHub API rate limit reached. Try again later or check Releases directly."
                .to_string(),
        ),
        Err(ureq::Error::Status(code, _)) => {
            UpdateCheckResult::Error(format!("GitHub returned HTTP {code}."))
        }
        Err(ureq::Error::Transport(e)) => {
            UpdateCheckResult::Error(format!("Network error while checking updates: {e}"))
        }
    }
}

pub fn evaluate_release_json(current_version: &str, json_body: &str) -> UpdateCheckResult {
    let payload: GithubReleasePayload = match serde_json::from_str(json_body) {
        Ok(p) => p,
        Err(e) => {
            return UpdateCheckResult::Error(format!("Invalid release metadata from GitHub: {e}"));
        }
    };

    let normalized_latest = normalize_version(&payload.tag_name);
    let release_url = if payload.html_url.trim().is_empty() {
        RELEASES_PAGE_URL.to_string()
    } else {
        payload.html_url.trim().to_string()
    };

    let installer_url = pick_installer_asset(&payload.assets);
    let notes_summary = summarize_notes(payload.body.as_deref().unwrap_or_default());
    let name = payload
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| payload.tag_name.clone());

    let release = ReleaseInfo {
        version: normalized_latest.clone(),
        tag_name: payload.tag_name,
        name,
        html_url: release_url.clone(),
        installer_url,
        published_at: payload.published_at,
        notes_summary,
    };

    if is_newer_version(current_version, &release.version) {
        UpdateCheckResult::UpdateAvailable {
            current: normalize_version(current_version),
            release,
        }
    } else {
        UpdateCheckResult::UpToDate {
            current: normalize_version(current_version),
            latest: normalized_latest,
            release_url,
        }
    }
}

pub fn normalize_version(raw: &str) -> String {
    raw.trim().trim_start_matches(['v', 'V']).trim().to_string()
}

pub fn parse_semver(raw: &str) -> Option<(u64, u64, u64, bool)> {
    let cleaned = normalize_version(raw);
    let without_build = cleaned.split('+').next().unwrap_or(&cleaned);
    let (core, has_prerelease) = match without_build.split_once('-') {
        Some((c, pre)) => (c, !pre.trim().is_empty()),
        None => (without_build, false),
    };

    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    Some((major, minor, patch, !has_prerelease))
}

pub fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some(cur), Some(lat)) => lat > cur,
        _ => false,
    }
}

fn pick_installer_asset(assets: &[GithubAssetPayload]) -> Option<String> {
    assets
        .iter()
        .find(|a| {
            let lower = a.name.to_ascii_lowercase();
            lower.ends_with(".exe") && lower.contains("setup")
        })
        .or_else(|| {
            assets
                .iter()
                .find(|a| a.name.to_ascii_lowercase().ends_with(".exe"))
        })
        .map(|a| a.browser_download_url.clone())
}

fn summarize_notes(raw: &str) -> String {
    let lines: Vec<&str> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("<!--"))
        .take(4)
        .collect();
    let joined = lines.join("\n");
    if joined.chars().count() > 260 {
        let prefix: String = joined.chars().take(259).collect();
        format!("{prefix}…")
    } else {
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semantic_versions_accurately() {
        assert!(is_newer_version("2.0.0", "2.0.1"));
        assert!(is_newer_version("2.0.0", "v2.1.0"));
        assert!(is_newer_version("2.0.0", "V3.0.0"));
        assert!(is_newer_version("2.0.0-beta.1", "2.0.0"));
        assert!(!is_newer_version("2.0.0", "2.0.0"));
        assert!(!is_newer_version("2.0.0", "v2.0.0"));
        assert!(!is_newer_version("2.1.0", "2.0.9"));
        assert!(!is_newer_version("2.0.0", "2.0.0-rc.1"));
    }

    #[test]
    fn parses_github_release_payload_and_finds_setup_installer() {
        let body = r###"{
            "tag_name": "v2.1.0",
            "name": "Kraken VPN v2.1.0",
            "html_url": "https://github.com/pandumalik/kraken-vpn/releases/tag/v2.1.0",
            "published_at": "2026-11-28T12:00:00Z",
            "body": "## Changes\n- Added update checker\n- Fixed tray menu",
            "assets": [
                {
                    "name": "SHA256SUMS.txt",
                    "browser_download_url": "https://example.com/SHA256SUMS.txt"
                },
                {
                    "name": "KrakenVPN-2.1.0-Setup.exe",
                    "browser_download_url": "https://example.com/KrakenVPN-2.1.0-Setup.exe"
                }
            ]
        }"###;

        let res = evaluate_release_json("2.0.0", body);
        match res {
            UpdateCheckResult::UpdateAvailable { current, release } => {
                assert_eq!(current, "2.0.0");
                assert_eq!(release.version, "2.1.0");
                assert_eq!(
                    release.installer_url.as_deref(),
                    Some("https://example.com/KrakenVPN-2.1.0-Setup.exe")
                );
                assert!(release.notes_summary.contains("Added update checker"));
            }
            other => panic!("expected UpdateAvailable, got {other:?}"),
        }

        let up_to_date = evaluate_release_json("2.1.0", body);
        assert!(matches!(up_to_date, UpdateCheckResult::UpToDate { .. }));
    }
}
