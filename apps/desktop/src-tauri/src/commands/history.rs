//! On-demand history, channels, chat cache and peer lookups.

use tauri::State;

use crate::state::{AppState, session};

#[tauri::command]
pub async fn fetch_chat_history(
    state: State<'_, AppState>,
    chat: String,
    oldest_id: String,
    oldest_from_me: bool,
    oldest_timestamp: i64,
    count: i32,
) -> Result<(), String> {
    session(&state)?
        .fetch_chat_history(&chat, &oldest_id, oldest_from_me, oldest_timestamp, count)
        .await
}

#[tauri::command]
pub async fn sync_channels(state: State<'_, AppState>) -> Result<usize, String> {
    session(&state)?.sync_channels().await
}

/// One page of a channel's posts; returns the `server_id` cursor for the next
/// (older) page, or `None` at the end.
#[tauri::command]
pub async fn fetch_channel_history(
    state: State<'_, AppState>,
    chat: String,
    name: String,
    count: u32,
    before: Option<u64>,
) -> Result<Option<u64>, String> {
    session(&state)?
        .fetch_channel_history(&chat, &name, count, before)
        .await
}

#[tauri::command]
pub async fn sync_chat_history(
    state: State<'_, AppState>,
    requests: Vec<wa_core::HistoryRequest>,
) -> Result<usize, String> {
    session(&state)?.sync_chat_history(&requests).await
}

#[tauri::command]
pub fn save_chat_cache(state: State<'_, AppState>, json: String) -> Result<(), String> {
    session(&state)?.save_chat_cache(&json)
}

#[tauri::command]
pub fn load_chat_cache(state: State<'_, AppState>) -> Option<String> {
    session(&state)
        .ok()
        .and_then(|session| session.load_chat_cache())
}

#[tauri::command]
pub async fn resolve_peers(
    state: State<'_, AppState>,
    chats: Vec<String>,
) -> Result<Vec<wa_core::PeerLookup>, String> {
    session(&state)?.resolve_peers(&chats).await
}
