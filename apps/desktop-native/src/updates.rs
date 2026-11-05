//! Self-update: find the newest GitHub release, download this platform's
//! binary and swap it in on restart.
//!
//! Every WA Desk release is published as a **pre-release**, so GitHub's
//! `/releases/latest` endpoint answers 404 and cannot be used here - the
//! updater lists `/releases` instead and picks the highest tag itself.

use std::path::{Path, PathBuf};

/// The repository that publishes the binaries.
pub const REPO: &str = "dirtTW/wa-desk";

/// The asset the release job uploads for this platform.
pub fn asset_name() -> &'static str {
    if cfg!(windows) {
        "wa-desk-native.exe"
    } else {
        "wa-desk-native"
    }
}

/// One newer release that has a downloadable asset for this platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub url: String,
    pub size: u64,
    pub notes_url: String,
}

/// What the updater is doing, for the Settings screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Checking,
    /// A newer release was found, nothing downloaded yet.
    Available(Release),
    Downloading(Release),
    /// The binary is on disk and waiting for the restart.
    Ready(PathBuf),
    Failed(String),
    /// Nothing newer exists.
    UpToDate,
}

/// Replies from the worker threads.
#[derive(Debug)]
pub enum UpdateMsg {
    Checked(Result<Option<Release>, String>),
    Downloaded(Result<PathBuf, String>),
}

/// Ask GitHub for the newest release, if it is newer than `current`.
pub fn check(current: &str) -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases?per_page=20");
    let mut response = ureq::get(&url)
        .header("User-Agent", "wa-desk-updater")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|err| format!("could not reach GitHub: {err}"))?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|err| format!("could not read the release list: {err}"))?;
    parse_releases(&text, current)
}

/// Pick the highest non-draft release that is newer than `current` and ships
/// this platform's asset.
pub fn parse_releases(json: &str, current: &str) -> Result<Option<Release>, String> {
    let releases: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|err| format!("unexpected release list: {err}"))?;
    let mut best: Option<Release> = None;
    for release in releases {
        if release["draft"].as_bool().unwrap_or(false) {
            continue;
        }
        let Some(tag) = release["tag_name"].as_str() else {
            continue;
        };
        if !is_newer(tag, current) {
            continue;
        }
        let asset = release["assets"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|asset| asset["name"].as_str() == Some(asset_name()));
        let Some(asset) = asset else {
            continue;
        };
        let url = asset["browser_download_url"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if url.is_empty() {
            continue;
        }
        let candidate = Release {
            tag: tag.to_string(),
            url,
            size: asset["size"].as_u64().unwrap_or(0),
            notes_url: release["html_url"].as_str().unwrap_or_default().to_string(),
        };
        if best
            .as_ref()
            .map(|current| version_key(&candidate.tag) > version_key(&current.tag))
            .unwrap_or(true)
        {
            best = Some(candidate);
        }
    }
    Ok(best)
}

/// Bytes written so far by the current update download, and the expected total
/// (`0` while the server has not said). The settings page reads these to paint
/// the progress bar under the button.
pub static DOWNLOAD_DONE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static DOWNLOAD_TOTAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How far the current download has got, `0.0..=1.0`, when a total is known.
pub fn download_fraction() -> Option<f32> {
    use std::sync::atomic::Ordering;
    let total = DOWNLOAD_TOTAL.load(Ordering::Relaxed);
    if total == 0 {
        return None;
    }
    let done = DOWNLOAD_DONE.load(Ordering::Relaxed);
    Some((done as f32 / total as f32).clamp(0.0, 1.0))
}

/// The download's byte counts as `(done, total)`, for the label.
pub fn download_bytes() -> (u64, u64) {
    use std::sync::atomic::Ordering;
    (
        DOWNLOAD_DONE.load(Ordering::Relaxed),
        DOWNLOAD_TOTAL.load(Ordering::Relaxed),
    )
}

/// Download the release binary into `dest`.
pub fn download(release: &Release, dest: &Path) -> Result<PathBuf, String> {
    let mut response = ureq::get(&release.url)
        .header("User-Agent", "wa-desk-updater")
        .call()
        .map_err(|err| format!("download failed: {err}"))?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    use std::sync::atomic::Ordering;
    DOWNLOAD_DONE.store(0, Ordering::Relaxed);
    let total = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    DOWNLOAD_TOTAL.store(total, Ordering::Relaxed);
    let mut file = std::fs::File::create(dest).map_err(|err| err.to_string())?;
    let mut reader = response.body_mut().as_reader();
    let mut buffer = [0u8; 64 * 1024];
    let mut done: u64 = 0;
    loop {
        let read = std::io::Read::read(&mut reader, &mut buffer)
            .map_err(|err| format!("download interrupted: {err}"))?;
        if read == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buffer[..read])
            .map_err(|err| format!("cannot write the update: {err}"))?;
        done += read as u64;
        DOWNLOAD_DONE.store(done, Ordering::Relaxed);
    }
    Ok(dest.to_path_buf())
}

/// Where a downloaded binary is staged before the swap.
pub fn staging_path() -> PathBuf {
    std::env::temp_dir().join(format!("wa-desk-update-{}", asset_name()))
}

/// Replace the running executable with `downloaded`, start it, and let the
/// caller quit. Windows cannot overwrite a running exe, but it can rename one,
/// so the current binary is set aside as `<name>.old.exe` first.
pub fn install(downloaded: &Path) -> Result<(), String> {
    let current = std::env::current_exe().map_err(|err| format!("where am I? {err}"))?;
    let staged = current.with_file_name(format!("{}.new", app_file_name(&current)));
    std::fs::copy(downloaded, &staged)
        .map_err(|err| format!("could not stage the update: {err}"))?;

    #[cfg(windows)]
    let old = current.with_file_name(format!("{}.old", app_file_name(&current)));

    #[cfg(windows)]
    {
        let _ = std::fs::remove_file(&old);
        std::fs::rename(&current, &old)
            .map_err(|err| format!("could not set the old binary aside: {err}"))?;
        std::fs::rename(&staged, &current)
            .map_err(|err| format!("could not put the new binary in place: {err}"))?;
    }

    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&staged)
            .map_err(|err| err.to_string())?
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&staged, permissions).map_err(|err| err.to_string())?;
        std::fs::rename(&staged, &current)
            .map_err(|err| format!("could not put the new binary in place: {err}"))?;
    }

    std::process::Command::new(&current)
        .arg("--restarted")
        .spawn()
        .map_err(|err| format!("could not restart: {err}"))?;
    Ok(())
}

/// The file name of the running executable (`WA Desk.exe` when renamed by the
/// user, `wa-desk-native.exe` in a plain build).
fn app_file_name(current: &Path) -> String {
    current
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "wa-desk-native".to_string())
}

/// Whether `latest` is a newer version than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    version_key(latest) > version_key(current)
}

/// Sort key: numeric core, then final (2) above any pre-release (1), then the
/// pre-release number. `0.2.0` > `0.2.0-rc.61` > `0.2.0-rc.60`.
fn version_key(tag: &str) -> (Vec<u64>, u8, u64) {
    let tag = tag.trim().trim_start_matches('v');
    let (core, pre) = match tag.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (tag, None),
    };
    let numbers: Vec<u64> = core
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    match pre {
        None => (numbers, 2, 0),
        Some(pre) => {
            let number = pre
                .rsplit('.')
                .next()
                .and_then(|part| part.parse().ok())
                .unwrap_or(0);
            (numbers, 1, number)
        }
    }
}

/// Fetch arbitrary bytes (a profile photo) with the same client as the updater.
pub fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    use std::io::Read as _;

    let mut response = ureq::get(url)
        .header("User-Agent", "wa-desk")
        .call()
        .map_err(|err| err.to_string())?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut bytes)
        .map_err(|err| err.to_string())?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prereleases_compare_in_order() {
        assert!(is_newer("v0.2.0-rc.61", "0.2.0-rc.60"));
        assert!(!is_newer("v0.2.0-rc.60", "0.2.0-rc.60"));
        assert!(!is_newer("v0.2.0-rc.59", "0.2.0-rc.60"));
        // A final release beats every pre-release of the same core.
        assert!(is_newer("v0.2.0", "0.2.0-rc.99"));
        assert!(is_newer("v0.3.0-rc.1", "0.2.0"));
        assert!(!is_newer("v0.2.0", "0.2.1"));
        // Past rc.99 the core moves on: 0.2.2-rc.1 ships after 0.2.1-rc.99, and
        // both orderings stay numeric (rc.100 would also beat rc.99).
        assert!(is_newer("v0.2.2-rc.1", "0.2.1-rc.99"));
        assert!(!is_newer("v0.2.1-rc.99", "0.2.2-rc.1"));
        assert!(is_newer("v0.2.1-rc.100", "0.2.1-rc.99"));
        // The rc number is compared as a number, not as text.
        assert!(is_newer("v0.2.0-rc.10", "0.2.0-rc.9"));
        assert!(!is_newer("v0.2.0-rc.9", "0.2.0-rc.10"));
    }

    #[test]
    fn a_bigger_core_version_updates_even_from_a_prerelease() {
        // 0.2.0-rc.60 -> 0.3.0-rc.1: a minor bump is an update.
        assert!(is_newer("v0.3.0-rc.1", "0.2.0-rc.60"));
        // ...and so is a major bump, or a final release on the next minor.
        assert!(is_newer("v1.0.0", "0.2.0-rc.60"));
        assert!(is_newer("v0.3.0", "0.2.0"));
        // The same core stays gated by the rc number.
        assert!(!is_newer("v0.2.0-rc.60", "0.2.0-rc.61"));
    }

    #[test]
    fn the_newest_release_with_a_platform_asset_wins() {
        // The asset name is picked at compile time, so the fixtures must use it
        // too (the `check` job runs on Linux and Windows).
        let platform = asset_name();
        let other = if platform == "wa-desk-native.exe" {
            "wa-desk-native"
        } else {
            "wa-desk-native.exe"
        };
        let json = serde_json::json!([
            {"tag_name": "v0.2.0-rc.62", "draft": false, "html_url": "x",
             "assets": [{"name": platform, "size": 10,
                         "browser_download_url": "https://example/newest"}]},
            {"tag_name": "v0.2.0-rc.61", "draft": false, "html_url": "y",
             "assets": [{"name": platform, "size": 9,
                         "browser_download_url": "https://example/old"}]},
            {"tag_name": "v0.2.0-rc.63", "draft": true, "html_url": "z",
             "assets": [{"name": platform, "size": 11,
                         "browser_download_url": "https://example/draft"}]},
            // Newer, published, but only ships the other platform's binary.
            {"tag_name": "v0.2.0-rc.64", "draft": false, "html_url": "w",
             "assets": [{"name": other, "size": 12,
                         "browser_download_url": "https://example/other-platform"}]}
        ])
        .to_string();

        let release = parse_releases(&json, "0.2.0-rc.60")
            .expect("a parseable list")
            .expect("a newer release");
        assert_eq!(release.tag, "v0.2.0-rc.62");
        assert_eq!(release.url, "https://example/newest");
        assert_eq!(release.size, 10);
    }

    #[test]
    fn an_up_to_date_app_gets_nothing() {
        let json = serde_json::json!([
            {"tag_name": "v0.2.0-rc.60", "draft": false,
             "assets": [{"name": asset_name(), "browser_download_url": "u"}]}
        ])
        .to_string();
        assert_eq!(parse_releases(&json, "0.2.0-rc.60").unwrap(), None);
        assert!(parse_releases("[]", "0.2.0-rc.60").unwrap().is_none());
        assert!(parse_releases("not json", "0.2.0-rc.60").is_err());
    }

    #[test]
    fn the_newest_core_wins_over_a_busier_rc_line() {
        let json = serde_json::json!([
            {"tag_name": "v0.2.0-rc.90", "draft": false, "html_url": "a",
             "assets": [{"name": asset_name(), "size": 10,
                         "browser_download_url": "https://example/rc90"}]},
            {"tag_name": "v0.3.0-rc.1", "draft": false, "html_url": "b",
             "assets": [{"name": asset_name(), "size": 11,
                         "browser_download_url": "https://example/030"}]}
        ])
        .to_string();
        let release = parse_releases(&json, "0.2.0-rc.60")
            .expect("parse")
            .expect("a newer release");
        assert_eq!(release.tag, "v0.3.0-rc.1");
        assert_eq!(release.url, "https://example/030");
    }
    #[test]
    fn the_asset_name_matches_the_release_job() {
        let expected = if cfg!(windows) {
            "wa-desk-native.exe"
        } else {
            "wa-desk-native"
        };
        assert_eq!(asset_name(), expected);
    }
}
