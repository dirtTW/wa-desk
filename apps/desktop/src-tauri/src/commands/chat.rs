//! Chat-level settings: archive, mute, pin, unread, block, clear, delete.
//!
//! All of them ride app-state mutations (or the blocklist IQ), which WhatsApp
//! syncs to the phone and every other linked client — the official desktop's
//! cross-device chat menus.

use tauri::State;

use crate::state::{AppState, session};

#[tauri::command]
pub async fn chat_archive(
    state: State<'_, AppState>,
    chat: String,
    archived: bool,
) -> Result<(), String> {
    session(&state)?.archive_chat(&chat, archived).await
}

#[tauri::command]
pub async fn chat_mute(
    state: State<'_, AppState>,
    chat: String,
    until_ms: i64,
) -> Result<(), String> {
    session(&state)?.mute_chat(&chat, until_ms).await
}

#[tauri::command]
pub async fn chat_pin(
    state: State<'_, AppState>,
    chat: String,
    pinned: bool,
) -> Result<(), String> {
    // Non-blocking: the patch runs in the background; the fn has no awaits on
    // the ack chain, so the invoke returns before the patch does.
    session(&state)?.pin_chat(&chat, pinned).await
}

#[tauri::command]
pub async fn chat_read_mark(
    state: State<'_, AppState>,
    chat: String,
    unread: bool,
) -> Result<(), String> {
    session(&state)?.mark_chat_unread(&chat, unread).await
}

#[tauri::command]
pub async fn chat_star(
    state: State<'_, AppState>,
    chat: String,
    id: String,
    starred: bool,
    from_me: bool,
    participant: Option<String>,
) -> Result<(), String> {
    session(&state)?
        .star_message(&chat, &id, starred, from_me, participant.as_deref())
        .await
}

#[tauri::command]
pub async fn chat_delete(state: State<'_, AppState>, chat: String) -> Result<(), String> {
    session(&state)?.delete_chat(&chat).await
}

#[tauri::command]
pub async fn chat_clear(state: State<'_, AppState>, chat: String) -> Result<(), String> {
    session(&state)?.clear_chat(&chat).await
}

#[tauri::command]
pub async fn chat_block(
    state: State<'_, AppState>,
    chat: String,
    blocked: bool,
) -> Result<(), String> {
    session(&state)?.block_contact(&chat, blocked).await
}
