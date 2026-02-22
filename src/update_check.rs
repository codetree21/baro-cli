use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CHECK_INTERVAL_SECS: u64 = 86400; // 24 hours
const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/codetree21/baro-cli/releases/latest";

#[derive(serde::Serialize, serde::Deserialize)]
struct CachedCheck {
    latest_version: String,
    checked_at: u64,
}

fn cache_path() -> Option<PathBuf> {
    config::config_dir().ok().map(|d| d.join("version-check.json"))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Spawn a background version check. Returns a handle that can be awaited
/// briefly at the end of main to print the update notice if available.
pub fn spawn_check() -> tokio::task::JoinHandle<Option<String>> {
    tokio::spawn(async {
        check_and_notify().await
    })
}

async fn check_and_notify() -> Option<String> {
    // Read cache
    let path = cache_path()?;
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(cached) = serde_json::from_str::<CachedCheck>(&data) {
            if now_secs() - cached.checked_at < CHECK_INTERVAL_SECS {
                return format_notice(&cached.latest_version);
            }
        }
    }

    // Fetch latest from GitHub
    let latest = fetch_latest_version().await?;

    // Write cache
    let cached = CachedCheck {
        latest_version: latest.clone(),
        checked_at: now_secs(),
    };
    if let Ok(json) = serde_json::to_string(&cached) {
        let _ = std::fs::write(&path, json);
    }

    format_notice(&latest)
}

async fn fetch_latest_version() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .ok()?;

    let resp = client
        .get(GITHUB_RELEASES_URL)
        .header("User-Agent", format!("baro-cli/{}", CURRENT_VERSION))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let body: serde_json::Value = resp.json().await.ok()?;
    let tag = body["tag_name"].as_str()?;
    Some(tag.trim_start_matches('v').to_string())
}

fn format_notice(latest: &str) -> Option<String> {
    if is_newer(latest, CURRENT_VERSION) {
        Some(format!(
            "\nUpdate available: v{} → v{}\n  Run: curl -fsSL https://raw.githubusercontent.com/codetree21/baro-cli/main/install.sh | sh",
            CURRENT_VERSION, latest
        ))
    } else {
        None
    }
}

fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split('.').filter_map(|s| s.parse().ok()).collect()
    };
    let l = parse(latest);
    let c = parse(current);
    l > c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_newer_patch() {
        assert!(is_newer("0.3.1", "0.3.0"));
    }

    #[test]
    fn is_newer_minor() {
        assert!(is_newer("0.4.0", "0.3.0"));
    }

    #[test]
    fn is_newer_major() {
        assert!(is_newer("1.0.0", "0.3.0"));
    }

    #[test]
    fn same_version_not_newer() {
        assert!(!is_newer("0.3.0", "0.3.0"));
    }

    #[test]
    fn older_not_newer() {
        assert!(!is_newer("0.2.0", "0.3.0"));
    }
}
