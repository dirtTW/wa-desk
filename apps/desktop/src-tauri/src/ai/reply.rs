//! The live event handler: decide whether a message gets an AI reply, build the
//! model request, send the answer and occasionally react or send a sticker.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use wa_core::UiEvent;

use super::media::{image_data_url, video_frames};
use super::prompt::{
    ReplyGate, add_guard, ai_prompt, build_turns, guard_message, identity_message,
    jailbreak_attempt, mentioned_me, own_user_parts, prompt_for_message, prompt_with_reply,
    reply_is_to_me, should_reply, sticker_request, strip_numeric_mentions,
};
use super::provider::{
    chat_completion, chat_with_model, select_sticker, sticker_candidates, vision_content,
};
use super::state::AiState;
use super::util::{contains_emoji, human_delay_ms, now_ms, random_below, strip_emoji};
use crate::state::AppState;

const HISTORY_TURNS: usize = 6;
const MIN_REPLY_GAP: Duration = Duration::from_secs(2);
const MIN_REACTION_GAP: Duration = Duration::from_secs(180);
const EMOJI_GAP: Duration = Duration::from_secs(120);
const STICKER_GAP: Duration = Duration::from_secs(180);
const STICKER_REQUEST_GAP: Duration = Duration::from_secs(20);
const STICKER_CANDIDATES: usize = 8;
const STICKER_REQUEST_CANDIDATES: usize = 12;

pub async fn handle_event(app: &AppHandle, event: &UiEvent) {
    let (message, is_history) = match event {
        UiEvent::Message(message) => (message, false),
        UiEvent::HistoryMessage(message) => (message, true),
        _ => return,
    };
    let ai = app.state::<AiState>();
    ai.record_context(message);
    if is_history {
        return;
    }
    let media = matches!(
        message.kind.as_str(),
        "image" | "video" | "document" | "sticker"
    );
    if message.kind != "text" && !media {
        return;
    }

    let text = message.text.trim();
    let Some(prompt) = prompt_for_message(&message.kind, text) else {
        return;
    };
    let prompt = prompt_with_reply(prompt, message.reply_to.as_ref());

    let session = {
        let app_state = app.state::<AppState>();
        app_state.session.lock().expect("session lock").clone()
    };
    let Some(session) = session else {
        return;
    };

    let self_chat = session.is_self_chat(&message.chat);
    if message.from_me && !self_chat {
        return;
    }
    if self_chat && ai_prompt(text).is_none() {
        return;
    }

    let triggered = ai_prompt(text).is_some();
    let wants_sticker = sticker_request(text);
    let (stored, key) = ai.config();
    if key.is_none() {
        return;
    }

    let now = now_ms();
    let chat = message.chat.clone();
    let mentioned = mentioned_me(text, &session, &message.mentions);
    let own = own_user_parts(&session);
    let replied_to_me = message
        .reply_to
        .as_ref()
        .is_some_and(|reply| reply_is_to_me(reply, &session, &own));

    let saved = self_chat || (!message.is_group && session.is_saved_contact(&chat).await);
    if !should_reply(ReplyGate {
        enabled: stored.enabled,
        triggered,
        require_trigger: stored.require_trigger,
        from_me: message.from_me,
        is_group: message.is_group,
        self_chat,
        saved_contact: saved,
        mentioned_me: mentioned,
        replied_to_me,
        sticker_request: wants_sticker,
        age_ms: now - message.timestamp,
    }) {
        return;
    }

    {
        let mut last = ai.last_reply.lock().expect("ai rate lock");
        if let Some(previous) = last.get(&chat)
            && previous.elapsed() < MIN_REPLY_GAP
        {
            return;
        }
        last.insert(chat.clone(), Instant::now());
    }

    let history_snapshot: Vec<(String, String)> = {
        let history = ai.history.lock().expect("ai history lock");
        history.get(&chat).cloned().unwrap_or_default()
    };
    let stickers: Vec<String> = ai.stickers.lock().expect("ai stickers lock").clone();
    let context = ai.context_block(&chat, &message.id);
    if let Some(context) = &context {
        tracing::debug!(chat = %chat, chars = context.len(), "AI context attached");
    }
    let mut images: Vec<String> = Vec::new();
    match message.kind.as_str() {
        "image" | "sticker" => {
            if let Some(image) = image_data_url(&session, &message.id).await {
                images.push(image);
            }
        }
        "video" => {
            images = video_frames(&session, &message.id).await;
            if images.is_empty()
                && let Some(thumb) = session.message_thumbnail(&message.id)
            {
                images.push(thumb);
            }
        }
        _ => {}
    }
    if images.is_empty()
        && let Some(reply) = &message.reply_to
        && !reply.id.is_empty()
    {
        match reply.kind.as_str() {
            "image" | "sticker" => {
                if let Some(image) = image_data_url(&session, &reply.id).await {
                    images.push(image);
                }
            }
            "video" => {
                images = video_frames(&session, &reply.id).await;
            }
            _ => {}
        }
        if images.is_empty()
            && let Some(thumb) = &reply.thumb
        {
            images.push(thumb.clone());
        }
    }
    if !images.is_empty() {
        tracing::debug!(
            chat = %chat,
            kind = %message.kind,
            count = images.len(),
            "AI vision input ready"
        );
    }

    let _ = session.set_typing(&chat, true).await;
    let api_key = key.clone().unwrap_or_default();
    let system_prompt = format!("{}\n\n{}", stored.system_prompt, identity_message(&session));
    let guard = jailbreak_attempt(text).then(|| guard_message(&session.push_name()));
    let request = {
        let stored = stored.clone();
        let request_key = api_key.clone();
        let session_id = ai.session_id.clone();
        let prompt = prompt.clone();
        let history = history_snapshot.clone();
        let context = context.clone();
        let system_prompt = system_prompt.clone();
        let guard = guard.clone();
        tokio::task::spawn_blocking(move || -> Result<String, String> {
            let key = request_key;
            if !images.is_empty() {
                let vision_model = if stored.vision_model.trim().is_empty() {
                    stored.model.as_str()
                } else {
                    stored.vision_model.as_str()
                };
                let mut turns = build_turns(&system_prompt, &history, context.as_deref(), &prompt);
                add_guard(&mut turns, guard.as_deref());
                if let Some(last) = turns.last_mut() {
                    last["content"] = vision_content(&prompt, &images);
                }
                match chat_with_model(&stored, &key, &session_id, vision_model, &turns) {
                    Ok(reply) if !reply.trim().is_empty() => return Ok(reply),
                    Ok(_) => {}
                    Err(err) => {
                        tracing::warn!(%err, "vision reply failed; falling back to text");
                    }
                }
            }
            let mut turns = build_turns(&system_prompt, &history, context.as_deref(), &prompt);
            add_guard(&mut turns, guard.as_deref());
            chat_completion(&stored, &key, &session_id, &turns)
        })
    }
    .await;
    let _ = session.set_typing(&chat, false).await;

    let raw = match request {
        Ok(Ok(raw)) => raw,
        Ok(Err(err)) => {
            tracing::warn!(%err, chat = %chat, "AI reply failed");
            return;
        }
        Err(err) => {
            tracing::warn!(%err, chat = %chat, "AI task failed");
            return;
        }
    };

    let output = super::prompt::parse_ai_output(&raw);
    let preferred_sticker = output.sticker.clone();
    let mut reply = strip_numeric_mentions(&output.reply).trim().to_owned();
    if reply.is_empty() {
        return;
    }

    if !self_chat {
        // Occasional reaction, at most one every few minutes per chat.
        if let Some(emoji) = output.reaction {
            let allow = {
                let mut last = ai.last_reaction.lock().expect("ai reaction lock");
                let ok = last
                    .get(&chat)
                    .is_none_or(|previous| previous.elapsed() >= MIN_REACTION_GAP);
                if ok {
                    last.insert(chat.clone(), Instant::now());
                }
                ok
            };
            if allow {
                let participant = if message.is_group {
                    message.sender.as_deref()
                } else {
                    None
                };
                if let Err(err) = session
                    .send_reaction(&chat, &message.id, false, participant, &emoji)
                    .await
                {
                    tracing::warn!(%err, chat = %chat, "AI reaction failed");
                } else {
                    tracing::info!(chat = %chat, emoji = %emoji, "AI reaction sent");
                }
            }
        }

        // Don't put an emoji in every reply: allow one every couple of minutes.
        if contains_emoji(&reply) {
            let allow = {
                let mut last = ai.last_emoji.lock().expect("ai emoji lock");
                let ok = last
                    .get(&chat)
                    .is_none_or(|previous| previous.elapsed() >= EMOJI_GAP);
                if ok {
                    last.insert(chat.clone(), Instant::now());
                }
                ok
            };
            if !allow {
                reply = strip_emoji(&reply);
                if reply.is_empty() {
                    return;
                }
            }
        }
    }

    {
        let mut history = ai.history.lock().expect("ai history lock");
        let entry = history.entry(chat.clone()).or_default();
        entry.push(("user".into(), prompt.to_owned()));
        entry.push(("assistant".into(), reply.clone()));
        let excess = entry.len().saturating_sub(HISTORY_TURNS * 2);
        if excess > 0 {
            entry.drain(0..excess);
        }
    }

    tracing::info!(chat = %chat, chars = reply.len(), "AI auto-reply");
    let delay = human_delay_ms(&reply, wants_sticker);
    let _ = session.set_typing(&chat, true).await;
    tokio::time::sleep(Duration::from_millis(delay)).await;
    ai.wait_for_send_slot().await;
    let _ = session.set_typing(&chat, false).await;
    let sent = session.send_text(&chat, &reply).await;
    if let Err(err) = &sent {
        tracing::warn!(%err, chat = %chat, "could not send AI reply");
    } else {
        // `id` is the server's message id: the webview binds its optimistic
        // bubble to it immediately (instead of a synthetic `ai-…` key), so
        // "delete for everyone" REVOKEs from any device can find the bubble.
        let _ = app.emit(
            "ai:reply",
            serde_json::json!({
                "chat": chat.clone(),
                "text": reply.clone(),
                "id": sent.clone().unwrap_or_default(),
            }),
        );
    }

    // Pick a sticker only after the text reply is out. The selector sees the
    // sticker images plus the recent conversation, so it can choose something
    // that actually fits instead of sending a random opaque ID.
    if sent.is_ok()
        && !self_chat
        && !stickers.is_empty()
        && (wants_sticker || random_below(if message.is_group { 2 } else { 5 }) == 0)
    {
        let gap = if wants_sticker {
            STICKER_REQUEST_GAP
        } else {
            STICKER_GAP
        };
        let allow = {
            let mut last = ai.last_sticker.lock().expect("ai sticker lock");
            let ok = last
                .get(&chat)
                .is_none_or(|previous| previous.elapsed() >= gap);
            if ok {
                last.insert(chat.clone(), Instant::now());
            }
            ok
        };
        if allow {
            let limit = if wants_sticker {
                STICKER_REQUEST_CANDIDATES
            } else {
                STICKER_CANDIDATES
            };
            let candidates = sticker_candidates(preferred_sticker.as_deref(), &stickers, limit);
            let mut situation = context.unwrap_or_default();
            situation.push_str(&format!("\nPesan masuk: {prompt}\nBalasan gw: {reply}"));
            if wants_sticker {
                situation.push_str(
                    "\nCatatan: lawan bicara minta stiker, pilih yang paling pas dan lucu.",
                );
            }
            if situation.chars().count() > 3_000 {
                situation = situation.chars().take(3_000).collect();
            }
            if let Some(sticker) = select_sticker(
                &session,
                &stored,
                &api_key,
                &ai.session_id,
                &candidates,
                &situation,
            )
            .await
            {
                match session.forward_sticker(&chat, &sticker).await {
                    Ok(()) => {
                        tracing::info!(
                            chat = %chat,
                            sticker = %sticker,
                            "AI sticker sent (context-aware)"
                        );
                    }
                    Err(err) => {
                        tracing::warn!(%err, chat = %chat, "AI sticker failed");
                    }
                }
            } else {
                tracing::debug!(chat = %chat, "no sticker matched the situation");
            }
        }
    }
}
