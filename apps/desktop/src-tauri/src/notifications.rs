//! Windows push notifications driven straight from the Rust event pump, so
//! the desktop keeps notifying when the webview is hidden, reloading or
//! throttled — the JS-side toast path could not guarantee that.
//!
//! The webview owns the parts Rust cannot see: the notification preference,
//! the open conversation and the mute states. It syncs them via
//! `set_notify_state` and this module decides on every incoming message.

use std::collections::HashSet;

use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::state::AppState;
use wa_core::{MessageView, UiEvent};

/// Notification-relevant state mirrored from the webview.
#[derive(Default)]
pub struct NotifyState {
    pub enabled: bool,
    pub active: Option<String>,
    pub muted: HashSet<String>,
}

#[derive(Default, Deserialize)]
pub struct NotifyPrefs {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    active: Option<String>,
    #[serde(default)]
    muted: Vec<String>,
}

pub fn set_prefs(state: &AppState, prefs: NotifyPrefs) {
    let mut stored = state.notify.lock().expect("notify lock");
    stored.enabled = prefs.enabled;
    stored.active = prefs.active.filter(|jid| !jid.is_empty());
    stored.muted = prefs.muted.into_iter().collect();
}

/// The toast identity: the Tauri identifier, used as the AppUserModelID.
#[cfg(windows)]
const AUMID: &str = "io.github.dirttw.wadesk";

/// Register the toast identity once per start. Unpackaged apps register
/// their AppUserModelID under `HKCU\Software\Classes\AppUserModelId` with a
/// `DisplayName` value — Windows then attributes toasts to "WA Desk".
/// Without it, toasts from an unregistered AUMID surface as
/// "Windows PowerShell", which is what the user saw.
#[cfg(windows)]
pub fn register_aumid() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let key = format!(r"HKCU\Software\Classes\AppUserModelId\{AUMID}");
    let run = |value: &str, data: &str| {
        let _ = std::process::Command::new("reg")
            .args(["add", &key, "/v", value, "/t", "REG_SZ", "/d", data, "/f"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    };
    run("DisplayName", "WA Desk");
    // An icon next to the exe, when the launcher provides one.
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let icon = dir.join("icon.ico");
        if icon.exists() {
            let uri = format!("file:///{}", icon.to_string_lossy().replace('\\', "/"));
            run("IconUri", &uri);
        }
    }
}
#[tauri::command]
pub async fn set_notify_state(
    app: AppHandle,
    prefs: NotifyPrefs,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    set_prefs(&state, prefs);
    Ok(())
}

/// Sidebar-parity preview: media kinds show their label with an optional
/// caption, exactly like `PREVIEW_LABELS` in the webview.
fn preview(view: &MessageView) -> String {
    let caption = view.text.trim();
    let label = match view.kind.as_str() {
        "text" => return view.text.clone(),
        "image" => "Photo",
        "video" => "Video",
        "audio" => "Audio",
        "document" => "Document",
        "sticker" => "Sticker",
        "view_once" => "View once message",
        _ => "Message",
    };
    if caption.is_empty() {
        label.to_owned()
    } else {
        format!("{label}: {caption}")
    }
}

/// Groups credit the sender, like the sidebar preview.
fn body_text(view: &MessageView) -> String {
    let body = preview(view);
    if view.is_group {
        match view.sender_name.as_deref() {
            Some(name) if !name.trim().is_empty() => format!("{name}: {body}"),
            _ => body,
        }
    } else {
        body
    }
}

pub fn handle_event(handle: &AppHandle, state: &AppState, event: &UiEvent) {
    let UiEvent::Message(view) = event else { return };
    // Our own sends (from any device) are not notification news.
    if view.from_me {
        return;
    }
    let (enabled, active, muted) = {
        let prefs = state.notify.lock().expect("notify lock");
        (
            prefs.enabled,
            prefs.active.clone(),
            prefs.muted.clone(),
        )
    };
    if !enabled {
        return;
    }
    if muted.contains(&view.chat) {
        return;
    }
    // The open conversation is auto-read, so it never notifies — same rule
    // the official client applies.
    if active.as_deref() == Some(view.chat.as_str()) {
        return;
    }
    show(handle, view);
}

fn show(handle: &AppHandle, view: &MessageView) {
    // The toast's own sound can be suppressed per-app or by focus assist, and
    // the official client pings on every message — play the system
    // notification sound explicitly, from the same gate as the toast.
    play_sound();
    let title = if view.chat_name.is_empty() {
        "New message".to_owned()
    } else {
        view.chat_name.clone()
    };
    if let Err(err) = handle
        .notification()
        .builder()
        .title(title)
        .body(body_text(view))
        .show()
    {
        tracing::warn!(%err, chat = %view.chat, "notification failed");
    }
}

/// Play the modern notification sound alias ("Notification.Default" — what
/// native toasts use) without blocking the pump.
#[cfg(windows)]
fn play_sound() {
    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{
        PlaySoundW, SND_ALIAS, SND_ASYNC, SND_FLAGS, SND_SYSTEM,
    };
    let alias: Vec<u16> = "Notification.Default\0".encode_utf16().collect();
    unsafe {
        let ok = PlaySoundW(
            PCWSTR(alias.as_ptr()),
            None,
            SND_FLAGS(SND_ALIAS.0 | SND_ASYNC.0 | SND_SYSTEM.0),
        );
        if !ok.as_bool() {
            tracing::warn!("notification sound failed to play");
        }
    }
}

#[cfg(not(windows))]
fn play_sound() {}

/// Diagnostic: push a sample message through the real event channel so the
/// whole Rust chain (pump → Windows toast) can be checked from the UI.
#[tauri::command]
pub async fn test_notify(_app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let tx = state
        .event_tx
        .lock()
        .expect("event tx lock")
        .clone()
        .ok_or_else(|| "the UI event channel is not ready".to_string())?;
    let view = MessageView {
        chat: "diagnostic@local".to_owned(),
        chat_name: "WA Desk".to_owned(),
        sender: None,
        sender_name: Some("Diagnostics".to_owned()),
        sender_phone: None,
        id: format!("diagnostic-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or_default()),
        from_me: false,
        is_group: false,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or_default(),
        kind: "text".to_owned(),
        text: "Windows push notifications are working".to_owned(),
        mentions: Vec::new(),
        reply_to: None,
        link: None,
        document: None,
        audio: None,
        stats: None,
        forwarded: false,
        once_recovered: false,
        status_quote: false,
    };
    tx.send(UiEvent::Message(view))
        .map_err(|err| format!("event channel closed: {err}"))
}
