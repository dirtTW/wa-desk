//! Message actions: reactions, deletes and edits.

use tauri::State;

use crate::state::{AppState, session};

#[tauri::command]
pub async fn send_reaction(
    state: State<'_, AppState>,
    chat: String,
    id: String,
    from_me: bool,
    participant: Option<String>,
    emoji: String,
) -> Result<(), String> {
    session(&state)?
        .send_reaction(&chat, &id, from_me, participant.as_deref(), &emoji)
        .await
}

#[tauri::command]
pub async fn delete_for_me(
    state: State<'_, AppState>,
    chat: String,
    id: String,
    from_me: bool,
    participant: Option<String>,
) -> Result<(), String> {
    session(&state)?
        .delete_for_me(&chat, &id, from_me, participant.as_deref())
        .await
        .inspect_err(|err| tracing::warn!(%err, "delete for me failed"))
}

#[tauri::command]
pub async fn delete_for_everyone(
    state: State<'_, AppState>,
    chat: String,
    id: String,
) -> Result<(), String> {
    session(&state)?
        .delete_for_everyone(&chat, &id)
        .await
        .inspect_err(|err| tracing::warn!(%err, "delete for everyone failed"))
}

#[tauri::command]
pub async fn edit_message(
    state: State<'_, AppState>,
    chat: String,
    id: String,
    text: String,
) -> Result<(), String> {
    session(&state)?
        .edit_message(&chat, &id, &text)
        .await
        .inspect_err(|err| tracing::warn!(%err, "edit failed"))
}
