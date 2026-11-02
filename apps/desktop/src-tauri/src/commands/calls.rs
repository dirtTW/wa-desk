//! 1:1 voice/video call signalling.

use tauri::State;

use crate::state::{AppState, session};

#[tauri::command]
pub async fn start_call(
    state: State<'_, AppState>,
    chat: String,
    video: bool,
) -> Result<wa_core::CallInfo, String> {
    session(&state)?.start_call(&chat, video).await
}

#[tauri::command]
pub async fn accept_call(
    state: State<'_, AppState>,
    call_id: String,
    video: bool,
) -> Result<wa_core::CallInfo, String> {
    session(&state)?.accept_call(&call_id, video).await
}

#[tauri::command]
pub async fn reject_call(state: State<'_, AppState>, call_id: String) -> Result<(), String> {
    session(&state)?.reject_call(&call_id).await
}

#[tauri::command]
pub async fn hangup_call(state: State<'_, AppState>) -> Result<(), String> {
    session(&state)?.hangup_call().await
}

#[tauri::command]
pub fn set_call_muted(state: State<'_, AppState>, muted: bool) -> Result<(), String> {
    session(&state)?.set_call_muted(muted)
}

#[tauri::command]
pub fn call_audio_frame(state: State<'_, AppState>, samples: Vec<i16>) -> Result<(), String> {
    session(&state)?.send_call_audio(samples)
}

#[tauri::command]
pub fn call_video_frame(state: State<'_, AppState>, data: Vec<u8>) -> Result<(), String> {
    session(&state)?.send_call_video(data)
}
