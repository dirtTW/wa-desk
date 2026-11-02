//! Chat-completion transport and the vision-based sticker picker.

use std::time::Duration;

use wa_core::Session;

use super::media::image_data_url;
use super::prompt::STICKER_SELECTOR_PROMPT;
use super::state::StoredAi;
use super::util::random_below;

pub(super) fn chat_completion(
    stored: &StoredAi,
    key: &str,
    session: &str,
    messages: &[serde_json::Value],
) -> Result<String, String> {
    chat_with_model(stored, key, session, &stored.model, messages)
}

pub(super) fn chat_with_model(
    stored: &StoredAi,
    key: &str,
    session: &str,
    model: &str,
    messages: &[serde_json::Value],
) -> Result<String, String> {
    let url = format!("{}/chat/completions", stored.base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": 0.7,
        // Reasoning models spend tokens before the visible answer; leave room
        // for image analysis plus a short chat reply.
        "max_tokens": 1200,
    });
    let body = serde_json::to_vec(&body).map_err(|e| e.to_string())?;

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .http_status_as_error(false)
        .user_agent("WA Desk/0.1")
        .build()
        .new_agent();

    let mut response = agent
        .post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        // OpenCode Zen (Go) routes requests per client session.
        .header("x-opencode-session", session)
        .send(body.as_slice())
        .map_err(|e| format!("AI request failed: {e}"))?;

    let status = response.status();
    let body = response
        .body_mut()
        .with_config()
        .limit(2_000_000)
        .read_to_string()
        .map_err(|e| format!("AI response unreadable: {e}"))?;

    if !status.is_success() {
        return Err(format!(
            "AI API returned {status}: {}",
            body.chars().take(300).collect::<String>()
        ));
    }

    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("AI response is not JSON: {e}"))?;
    json["choices"][0]["message"]["content"]
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "AI response had no message content: {}",
                body.chars().take(300).collect::<String>()
            )
        })
}

pub(super) fn vision_content(prompt: &str, images: &[String]) -> serde_json::Value {
    let mut content = vec![serde_json::json!({
        "type": "text",
        "text": prompt
    })];
    for image in images {
        content.push(serde_json::json!({
            "type": "image_url",
            "image_url": { "url": image }
        }));
    }
    serde_json::Value::Array(content)
}

pub(super) fn sticker_candidates(
    preferred: Option<&str>,
    stickers: &[String],
    limit: usize,
) -> Vec<String> {
    let mut pool = stickers.to_vec();
    for index in (1..pool.len()).rev() {
        let swap_with = random_below(index + 1);
        pool.swap(index, swap_with);
    }
    if let Some(preferred) = preferred.filter(|id| stickers.iter().any(|known| known == id)) {
        pool.retain(|id| id != preferred);
        pool.insert(0, preferred.to_owned());
    }
    pool.truncate(limit);
    pool
}

pub(super) fn parse_sticker_choice(raw: &str, candidates: &[String]) -> Option<String> {
    let trimmed = raw.trim();
    let cleaned = trimmed
        .strip_prefix("```json")
        .unwrap_or(trimmed)
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let value: serde_json::Value = serde_json::from_str(cleaned).ok()?;
    let id = value
        .get("sticker")
        .and_then(|value| value.as_str())?
        .trim();
    candidates
        .iter()
        .find(|candidate| candidate.as_str() == id)
        .cloned()
}

pub(super) async fn select_sticker(
    session: &Session,
    stored: &StoredAi,
    key: &str,
    session_id: &str,
    candidates: &[String],
    situation: &str,
) -> Option<String> {
    let mut parts = vec![serde_json::json!({
        "type": "text",
        "text": format!("{STICKER_SELECTOR_PROMPT}\n\nSITUASI CHAT:\n{situation}")
    })];
    let mut valid = Vec::new();
    for id in candidates {
        if let Some(image) = image_data_url(session, id).await {
            parts.push(serde_json::json!({
                "type": "text",
                "text": format!("ID stiker: {id}")
            }));
            parts.push(serde_json::json!({
                "type": "image_url",
                "image_url": { "url": image }
            }));
            valid.push(id.clone());
        }
    }
    if valid.len() < 2 {
        return None;
    }

    let model = if stored.vision_model.trim().is_empty() {
        stored.model.clone()
    } else {
        stored.vision_model.clone()
    };
    let stored = stored.clone();
    let key = key.to_owned();
    let session_id = session_id.to_owned();
    let messages = vec![
        serde_json::json!({ "role": "system", "content": STICKER_SELECTOR_PROMPT }),
        serde_json::json!({ "role": "user", "content": parts }),
    ];
    let raw = tokio::task::spawn_blocking(move || {
        chat_with_model(&stored, &key, &session_id, &model, &messages)
    })
    .await
    .ok()?
    .ok()?;
    parse_sticker_choice(&raw, &valid)
}
