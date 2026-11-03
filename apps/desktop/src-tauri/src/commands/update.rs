//! In-app update check and install, backed by `tauri-plugin-updater`.
//!
//! The plugin reads `plugins.updater.endpoints` — the `latest.json` attached to
//! the newest GitHub release — and verifies the downloaded installer against
//! the minisign public key in `plugins.updater.pubkey`.

use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::UpdaterExt;

/// What the settings screen shows after a check.
#[derive(Debug, Serialize)]
pub struct UpdateCheck {
    /// The version this build is running.
    pub current: String,
    /// The newer version, when one is published.
    pub version: Option<String>,
    /// The release notes, when the release carries them.
    pub notes: Option<String>,
    /// The release date, when the release carries one.
    pub date: Option<String>,
}

/// Ask the update endpoint whether a newer build exists.
#[tauri::command]
pub async fn check_for_update<R: Runtime>(app: AppHandle<R>) -> Result<UpdateCheck, String> {
    let current = app.package_info().version.to_string();
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(match update {
        Some(update) => UpdateCheck {
            current,
            version: Some(update.version.clone()),
            notes: update.body.clone(),
            date: update.date.map(|date| date.to_string()),
        },
        None => UpdateCheck {
            current,
            version: None,
            notes: None,
            date: None,
        },
    })
}

/// Download and install the pending update, then restart into it.
#[tauri::command]
pub async fn install_update<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let Some(update) = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
    else {
        return Err("this build is already the newest version".to_string());
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}
