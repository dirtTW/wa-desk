//! Session lifecycle, connection state and plain text chat handling.

use tauri::State;

use crate::state::{AppState, Status, session, start_session};

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> Status {
    let guard = state.session.lock().expect("session lock");
    let status = match guard.as_ref() {
        Some(session) => Status {
            ready: true,
            linked: session.is_paired(),
            connected: session.is_connected(),
            me: Some(session.push_name()).filter(|name| !name.is_empty()),
            jid: session.own_jid(),
            lid: session.own_lid(),
        },
        None => Status {
            ready: false,
            linked: false,
            connected: false,
            me: None,
            jid: None,
            lid: None,
        },
    };
    tracing::debug!(?status, "status requested");
    status
}

#[tauri::command]
pub async fn send_text(
    state: State<'_, AppState>,
    chat: String,
    text: String,
    reply_to: Option<String>,
    quoted_text: Option<String>,
) -> Result<(), String> {
    let session = session(&state)?;
    match reply_to {
        Some(id) => {
            session
                .send_text_reply(&chat, &text, &id, quoted_text.as_deref())
                .await
        }
        None => session.send_text(&chat, &text).await.map(|_| ()),
    }
}

/// Forward one cached message to the selected chats.
#[tauri::command]
pub async fn forward_message(
    state: State<'_, AppState>,
    chats: Vec<String>,
    id: String,
    text: Option<String>,
) -> Result<usize, String> {
    let session = session(&state)?;
    match session.forward_message(&chats, &id).await {
        Ok(sent) => Ok(sent),
        // Messages cached before bodies were stored can still be forwarded as
        // their text, with the forwarded marker on the copy.
        Err(err) => match text {
            Some(text) => session.forward_text(&chats, &text).await,
            None => Err(err),
        },
    }
}

#[tauri::command]
pub async fn mark_read(state: State<'_, AppState>, chat: String) -> Result<(), String> {
    session(&state)?.mark_read(&chat).await
}

#[tauri::command]
pub async fn mark_read_messages(
    state: State<'_, AppState>,
    chat: String,
    ids: Vec<String>,
) -> Result<(), String> {
    session(&state)?.mark_read_messages(&chat, ids).await
}

#[tauri::command]
pub async fn set_typing(
    state: State<'_, AppState>,
    chat: String,
    typing: bool,
) -> Result<(), String> {
    session(&state)?.set_typing(&chat, typing).await
}

#[tauri::command]
pub async fn request_pair_code(
    state: State<'_, AppState>,
    phone: String,
) -> Result<String, String> {
    session(&state)?.request_pair_code(&phone).await
}

#[tauri::command]
pub async fn reconnect_session(state: State<'_, AppState>) -> Result<(), String> {
    session(&state)?.reconnect_safely().await
}

#[tauri::command]
pub async fn restart_session(state: State<'_, AppState>) -> Result<(), String> {
    let old = state.session.lock().expect("session lock").take();
    if let Some(old) = old {
        old.shutdown().await;
    }
    start_session(state.inner()).await
}

#[tauri::command]
pub async fn set_push_name(state: State<'_, AppState>, name: String) -> Result<(), String> {
    tracing::info!("update push name");
    session(&state)?.set_push_name(&name).await
}

#[tauri::command]
pub async fn set_status_text(state: State<'_, AppState>, text: String) -> Result<(), String> {
    tracing::info!("update about text");
    session(&state)?.set_status_text(&text).await
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<(), String> {
    // Deregister on the server. The local teardown (store wipe + fresh session,
    // so the login screen gets a QR) is driven by the `LoggedOut` event in the
    // pump, which is the same path the phone unlinking us takes.
    session(&state)?.logout().await;
    Ok(())
}
