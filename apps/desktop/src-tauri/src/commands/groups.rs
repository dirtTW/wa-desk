//! Group creation, metadata and member management.

use tauri::State;

use crate::state::{AppState, session};

#[tauri::command]
pub async fn create_group(
    state: State<'_, AppState>,
    subject: String,
    participants: Vec<String>,
) -> Result<String, String> {
    session(&state)?.create_group(&subject, &participants).await
}

#[tauri::command]
pub async fn group_info(
    state: State<'_, AppState>,
    chat: String,
) -> Result<wa_core::GroupInfoView, String> {
    session(&state)?.group_info(&chat).await
}

#[tauri::command]
pub async fn leave_group(state: State<'_, AppState>, chat: String) -> Result<(), String> {
    session(&state)?.leave_group(&chat).await
}

#[tauri::command]
pub async fn group_add_members(
    state: State<'_, AppState>,
    chat: String,
    participants: Vec<String>,
) -> Result<(), String> {
    session(&state)?
        .add_group_members(&chat, &participants)
        .await
}

#[tauri::command]
pub async fn group_remove_members(
    state: State<'_, AppState>,
    chat: String,
    participants: Vec<String>,
) -> Result<(), String> {
    session(&state)?
        .remove_group_members(&chat, &participants)
        .await
}

#[tauri::command]
pub async fn group_promote_members(
    state: State<'_, AppState>,
    chat: String,
    participants: Vec<String>,
) -> Result<(), String> {
    session(&state)?
        .promote_group_members(&chat, &participants)
        .await
}

#[tauri::command]
pub async fn group_demote_members(
    state: State<'_, AppState>,
    chat: String,
    participants: Vec<String>,
) -> Result<(), String> {
    session(&state)?
        .demote_group_members(&chat, &participants)
        .await
}
