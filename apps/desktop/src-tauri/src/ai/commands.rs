//! The `#[tauri::command]` surface used by the settings screen and composer.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};

use super::prompt::{
    ai_prompt, default_prompt, guard_message, identity_message, jailbreak_attempt, parse_ai_output,
};
use super::provider::chat_completion;
use super::state::{
    AiContextLine, AiSettingsInput, AiSettingsView, AiState, default_base, default_model, encrypt,
};
use super::util::human_delay_ms;
use crate::state::AppState;

#[tauri::command]
pub fn get_ai_settings(state: State<'_, AiState>) -> AiSettingsView {
    state.view()
}

#[tauri::command]
pub fn save_ai_settings(
    state: State<'_, AiState>,
    settings: AiSettingsInput,
) -> Result<AiSettingsView, String> {
    let mut stored = state.stored.lock().expect("ai settings lock");

    if let Some(key) = settings
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
    {
        let enc = encrypt(&state.key, key)?;
        stored.key_enc = Some(enc);
        *state.api_key.lock().expect("ai key lock") = Some(key.to_owned());
    }

    stored.base_url = if settings.base_url.trim().is_empty() {
        default_base()
    } else {
        settings.base_url.trim().trim_end_matches('/').to_owned()
    };
    stored.model = if settings.model.trim().is_empty() {
        default_model()
    } else {
        settings.model.trim().to_owned()
    };
    stored.vision_model = settings.vision_model.trim().to_owned();
    stored.system_prompt = if settings.system_prompt.trim().is_empty() {
        default_prompt()
    } else {
        settings.system_prompt.trim().to_owned()
    };
    stored.require_trigger = settings.require_trigger;
    stored.enabled = settings.enabled;

    state.persist(&stored)?;
    drop(stored);
    Ok(state.view())
}

#[tauri::command]
pub fn set_ai_enabled(state: State<'_, AiState>, enabled: bool) -> Result<AiSettingsView, String> {
    if enabled && state.api_key.lock().expect("ai key lock").is_none() {
        return Err("add an API key before turning AI mode on".into());
    }
    {
        let mut stored = state.stored.lock().expect("ai settings lock");
        stored.enabled = enabled;
        state.persist(&stored)?;
    }
    Ok(state.view())
}

#[tauri::command]
pub fn set_ai_stickers(state: State<'_, AiState>, ids: Vec<String>) {
    let ids: Vec<String> = ids
        .into_iter()
        .filter(|id| !id.trim().is_empty())
        .take(80)
        .collect();
    tracing::debug!(count = ids.len(), "AI sticker list updated");
    *state.stickers.lock().expect("ai stickers lock") = ids;
}

/// UI-side seed of the last few visible messages for a chat. Keeps the AI
/// grounded after a restart, when live events alone would start empty.
#[tauri::command]
pub fn set_ai_context(state: State<'_, AiState>, chat: String, lines: Vec<AiContextLine>) {
    let added = lines.len();
    state.seed_context(&chat, lines);
    tracing::debug!(
        chat = %chat,
        added,
        "AI chat context seeded"
    );
}

#[tauri::command]
pub async fn test_ai(state: State<'_, AiState>) -> Result<String, String> {
    let (stored, key) = state.config();
    let key = key.ok_or("add an API key first")?;
    let session = state.session_id.clone();
    let messages = vec![
        serde_json::json!({ "role": "system", "content": stored.system_prompt }),
        serde_json::json!({
            "role": "user",
            "content": "Sapa gw singkat buat konfirmasi koneksi."
        }),
    ];
    tokio::task::spawn_blocking(move || chat_completion(&stored, &key, &session, &messages))
        .await
        .map_err(|e| e.to_string())?
}

/// Generate a reply for a saved contact without sending it. Used by the
/// settings screen to preview the assistant before enabling auto-reply.
#[tauri::command]
pub async fn ai_draft_reply(
    app: AppHandle,
    state: State<'_, AiState>,
    chat: String,
    text: String,
) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("write a message to preview".into());
    }

    let session = {
        let app_state = app.state::<AppState>();
        app_state.session.lock().expect("session lock").clone()
    }
    .ok_or("the WhatsApp session is still starting")?;

    if !session.is_saved_contact(&chat).await {
        return Err("AI replies are only sent to saved contacts".into());
    }

    let (stored, key) = state.config();
    let key = key.ok_or("add an API key first")?;
    let session_id = state.session_id.clone();
    let context = state.context_block(&chat, "");
    if let Some(context) = &context {
        tracing::debug!(chat = %chat, chars = context.len(), "AI context attached (preview)");
    }
    let mut messages = vec![serde_json::json!({
        "role": "system",
        "content": format!("{}\n\n{}", stored.system_prompt, identity_message(&session))
    })];
    if jailbreak_attempt(text) {
        messages.push(serde_json::json!({
            "role": "system",
            "content": guard_message(&session.push_name())
        }));
    }
    if let Some(context) = context {
        messages.push(serde_json::json!({
            "role": "system",
            "content": format!(
                "Konteks chat terakhir (buat paham obrolan, jangan dibalas semua):\n{context}"
            )
        }));
    }
    messages.push(serde_json::json!({ "role": "user", "content": text }));
    let raw =
        tokio::task::spawn_blocking(move || chat_completion(&stored, &key, &session_id, &messages))
            .await
            .map_err(|e| e.to_string())??;
    Ok(parse_ai_output(&raw).reply)
}

/// Reply to an `/ai` prompt sent from this client's composer. Self-sent
/// messages are not echoed back as events, so the UI calls this explicitly.
#[tauri::command]
pub async fn ai_reply_now(
    app: AppHandle,
    state: State<'_, AiState>,
    chat: String,
    text: String,
) -> Result<String, String> {
    let prompt = ai_prompt(text.trim()).ok_or("not an /ai prompt")?;

    let session = {
        let app_state = app.state::<AppState>();
        app_state.session.lock().expect("session lock").clone()
    }
    .ok_or("the WhatsApp session is still starting")?;

    if !session.is_self_chat(&chat) {
        return Err("composer /ai replies only work in Message yourself".into());
    }

    let (stored, key) = state.config();
    let key = key.ok_or("add an AI API key first")?;
    let session_id = state.session_id.clone();
    let messages = vec![
        serde_json::json!({
            "role": "system",
            "content": format!("{}\n\n{}", stored.system_prompt, identity_message(&session))
        }),
        serde_json::json!({
            "role": "system",
            "content": if jailbreak_attempt(prompt) { guard_message(&session.push_name()) } else { String::new() }
        }),
        serde_json::json!({ "role": "user", "content": prompt }),
    ];
    let raw =
        tokio::task::spawn_blocking(move || chat_completion(&stored, &key, &session_id, &messages))
            .await
            .map_err(|e| e.to_string())??;
    let reply = parse_ai_output(&raw).reply;

    let delay = human_delay_ms(&reply, false);
    let _ = session.set_typing(&chat, true).await;
    tokio::time::sleep(Duration::from_millis(delay)).await;
    state.wait_for_send_slot().await;
    let _ = session.set_typing(&chat, false).await;
    let sent_id = session
        .send_text(&chat, &reply)
        .await
        .map_err(|e| format!("could not send the AI reply: {e}"))?;
    let _ = app.emit(
        "ai:reply",
        serde_json::json!({ "chat": chat, "text": reply, "id": sent_id }),
    );
    Ok(reply)
}
