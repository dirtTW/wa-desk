//! Shared application state plus the WhatsApp session lifecycle helpers.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::Manager;
use wa_core::{Session, UiEvent};

#[derive(Default)]
pub struct AppState {
    pub session: Mutex<Option<Arc<Session>>>,
    /// Device store path, kept so a fresh session can be opened after logout.
    pub db_path: Mutex<Option<String>>,
    /// Media directory, kept for the same restart.
    pub media_dir: Mutex<Option<std::path::PathBuf>>,
    /// UI event sender; a restarted session reuses the same webview channel.
    pub event_tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<UiEvent>>>,
    /// Notification prefs mirrored from the webview (`notifications.rs`).
    pub notify: Mutex<crate::notifications::NotifyState>,
    /// Set once shutdown started, so `prevent_exit` cannot loop.
    pub exiting: AtomicBool,
}

#[derive(Debug, Serialize)]
pub struct Status {
    /// The session object exists (store opened, client built).
    pub ready: bool,
    /// Device credentials are present in the store.
    pub linked: bool,
    /// Transport is up right now.
    pub connected: bool,
    /// The account's push name, when known.
    pub me: Option<String>,
    /// The account's own WhatsApp id, when known.
    pub jid: Option<String>,
    /// The account's LID, used for its own profile picture.
    pub lid: Option<String>,
}

pub fn session(state: &AppState) -> Result<Arc<Session>, String> {
    state
        .session
        .lock()
        .expect("session lock")
        .clone()
        .ok_or_else(|| "the WhatsApp session is still starting".to_string())
}

/// Open a brand-new session on the stored paths, replacing the current one.
///
/// This is what makes pairing work after an in-app logout: `logout` clears the
/// device credentials, but the old supervisor is gone. Without a fresh start
/// the login screen would keep its spinner because nobody emits a QR any more.
pub async fn start_session(state: &AppState) -> Result<(), String> {
    let db_path = state
        .db_path
        .lock()
        .expect("db path lock")
        .clone()
        .ok_or_else(|| "the device store path is not ready".to_string())?;
    let media_dir = state
        .media_dir
        .lock()
        .expect("media dir lock")
        .clone()
        .ok_or_else(|| "the media directory is not ready".to_string())?;
    let tx = state
        .event_tx
        .lock()
        .expect("event tx lock")
        .clone()
        .ok_or_else(|| "the UI event channel is not ready".to_string())?;

    let session = Session::start(&db_path, media_dir, tx)
        .await
        .map_err(|err| format!("cannot start WhatsApp session: {err}"))?;
    *state.session.lock().expect("session lock") = Some(session);
    Ok(())
}

/// Remove the SQLite device store so the next session is unpaired.
///
/// `Client::logout` only deregisters the companion and disconnects; it leaves
/// the local credentials on disk — the vendor's own docs say to delete the
/// storage backend to clear them. Windows refuses to unlink a file another
/// handle still holds, and the protocol workers can outlive `shutdown()` by a
/// moment, so retry before giving up: a failed wipe is what left the app on
/// "Session ended (LoggedOut)" with dead credentials and no QR.
pub async fn clear_device_store(db_path: &str) -> Result<(), String> {
    let mut last_error = String::new();
    for attempt in 0..24 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
        last_error.clear();
        for suffix in ["", "-wal", "-shm"] {
            let path = format!("{db_path}{suffix}");
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    last_error = format!("cannot remove {path}: {err}");
                    break;
                }
            }
        }
        if last_error.is_empty() {
            return Ok(());
        }
    }
    Err(last_error)
}

/// The device store path for a generation. Generation 0 keeps the historical
/// `whatsapp.db` name so existing installs are untouched.
pub fn store_path(dir: &std::path::Path, generation: u32) -> std::path::PathBuf {
    if generation == 0 {
        dir.join("whatsapp.db")
    } else {
        dir.join(format!("whatsapp-{generation}.db"))
    }
}

fn generation_file(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("store.gen")
}

/// Which store the app is on. Bumped by a logout whose wipe could not unlink the
/// old file (a protocol worker still held it), so the next session opens a
/// brand-new store instead of replaying dead credentials.
pub fn store_generation(dir: &std::path::Path) -> u32 {
    std::fs::read_to_string(generation_file(dir))
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0)
}

pub fn bump_store_generation(dir: &std::path::Path) -> u32 {
    let next = store_generation(dir).saturating_add(1);
    let _ = std::fs::write(generation_file(dir), next.to_string());
    next
}

/// Delete every store file that is not the active one, plus any leftover
/// logout markers. Called at startup, when nothing holds those files.
pub fn purge_stale_stores(dir: &std::path::Path, active: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let active_name = active.file_name().map(|name| name.to_os_string());
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let is_store = name.starts_with("whatsapp") && name.ends_with(".db")
            || name.ends_with(".db-wal")
            || name.ends_with(".db-shm")
            || name.ends_with(".unlink");
        if !is_store {
            continue;
        }
        // Keep the active store and its sidecars.
        if let Some(active_name) = &active_name {
            let active_name = active_name.to_string_lossy();
            if name == active_name
                || name == format!("{active_name}-wal")
                || name == format!("{active_name}-shm")
            {
                continue;
            }
        }
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::info!(%name, "purged a store left by a previous run"),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => tracing::warn!(%err, %name, "could not purge a stale store"),
        }
    }
}

/// Tear a logged-out session down, wipe its credentials and open a fresh one so
/// the login screen gets a pairing QR instead of spinning forever.
///
/// Returns `false` when the store could not be removed: starting a session on
/// dead credentials would just be logged out again, so the caller leaves the
/// login screen alone and the next start purges the store.
pub async fn restart_after_logout(handle: &tauri::AppHandle) -> bool {
    let state = handle.state::<AppState>();
    let session = state.session.lock().expect("session lock").take();
    if let Some(session) = session {
        session.shutdown().await;
        drop(session);
    }
    let Some(db_path) = state.db_path.lock().expect("db path lock").clone() else {
        return false;
    };
    if let Err(err) = clear_device_store(&db_path).await {
        // The old store's SQLite handle outlived the session (protocol workers
        // keep the client alive), and Windows will not unlink an open file.
        // Move to a fresh generation instead: the next session opens a store
        // with no credentials, so a pairing QR appears, and the old file is
        // purged at the following start.
        tracing::warn!(%err, "device store is still locked; switching to a fresh store");
        let old = std::path::PathBuf::from(&db_path);
        let Some(dir) = old.parent().map(std::path::Path::to_path_buf) else {
            return false;
        };
        let generation = bump_store_generation(&dir);
        let fresh = store_path(&dir, generation);
        tracing::info!(path = %fresh.display(), generation, "opening a fresh device store");
        *state.db_path.lock().expect("db path lock") = Some(fresh.to_string_lossy().into_owned());
    }
    match start_session(state.inner()).await {
        Ok(()) => {
            tracing::info!("device store wiped; waiting for a fresh pairing QR");
            true
        }
        Err(err) => {
            tracing::error!(%err, "could not reopen the session after logout");
            false
        }
    }
}

pub fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
