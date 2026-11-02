//! Address book: reading, syncing and creating contacts.

use tauri::State;

use crate::state::{AppState, session};

#[tauri::command]
pub async fn get_contacts(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    Ok(session(&state)?.contacts())
}

#[tauri::command]
pub async fn sync_contacts(state: State<'_, AppState>) -> Result<(), String> {
    tracing::info!("sync contacts from the phone address book");
    session(&state)?.sync_contacts().await
}

#[tauri::command]
pub async fn save_contact(
    state: State<'_, AppState>,
    phone: String,
    name: String,
) -> Result<String, String> {
    session(&state)?.save_contact(&phone, &name).await
}

#[tauri::command]
pub async fn resolve_phone(
    state: State<'_, AppState>,
    jid: String,
) -> Result<Option<String>, String> {
    let session = session(&state)?;
    Ok(session.resolve_phone(&jid).await)
}
