//! Status / stories: posting, history and interactions.

use tauri::State;

use crate::state::{AppState, session};

#[tauri::command]
pub async fn post_status_text(
    state: State<'_, AppState>,
    text: String,
    background: Option<u32>,
    font: Option<String>,
) -> Result<String, String> {
    session(&state)?
        .post_status_text(
            &text,
            background.unwrap_or(0xFF00_A884),
            font.as_deref().unwrap_or("system"),
        )
        .await
}

#[tauri::command]
pub async fn post_status_media(
    state: State<'_, AppState>,
    data: String,
    mime: String,
    caption: String,
    thumb: String,
    duration: u32,
) -> Result<(), String> {
    session(&state)?
        .post_status_media(&data, &mime, &caption, &thumb, duration)
        .await
}

#[tauri::command]
pub async fn fetch_status_history(state: State<'_, AppState>) -> Result<(), String> {
    session(&state)?.fetch_status_history().await
}

#[tauri::command]
pub async fn mark_status_viewed(
    state: State<'_, AppState>,
    sender: String,
    id: String,
) -> Result<(), String> {
    session(&state)?.mark_status_viewed(&sender, &id).await
}

#[tauri::command]
pub async fn react_status(
    state: State<'_, AppState>,
    sender: String,
    id: String,
    emoji: String,
) -> Result<(), String> {
    session(&state)?.react_status(&sender, &id, &emoji).await
}

#[tauri::command]
pub async fn reply_status(
    state: State<'_, AppState>,
    sender: String,
    id: String,
    text: String,
    status_text: Option<String>,
) -> Result<(), String> {
    session(&state)?
        .reply_status(&sender, &id, &text, status_text.as_deref())
        .await
}

#[tauri::command]
pub async fn delete_status(state: State<'_, AppState>, id: String) -> Result<(), String> {
    session(&state)?.delete_status(&id).await
}

/// Tell the session which status ids belong to this account, so incoming
/// `<receipt type="view">` stanzas can be attributed to a story and surfaced as
/// "who viewed my status". The UI owns the status list, so it pushes the ids on
/// startup and after every post.
#[tauri::command]
pub fn watch_status_ids(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    session(&state)?.remember_own_statuses(ids);
    Ok(())
}
