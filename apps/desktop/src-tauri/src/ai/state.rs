//! Persisted AI settings, the encrypted API key and the bounded chat context.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use aes_gcm::aead::{Aead, AeadCore, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};

use super::prompt::{
    LEGACY_PROMPT, LEGACY_PROMPT_ID, LEGACY_PROMPT_ID2, LEGACY_PROMPT_ID3, LEGACY_PROMPT_ID4,
    default_prompt,
};
use super::util::{media_label, new_session_id, now_ms, user_part};

const DEFAULT_BASE: &str = "https://opencode.ai/zen/go/v1";
const DEFAULT_MODEL: &str = "deepseek-v4.1-flash";

/// Minimum gap between two sends, so bursty replies do not look like spam.
pub(super) const GLOBAL_SEND_GAP: Duration = Duration::from_secs(5);

pub(super) const CONTEXT_LINES: usize = 24;
pub(super) const CONTEXT_CHARS: usize = 4_000;
pub(super) const CONTEXT_AGE_MS: i64 = 12 * 60 * 60 * 1000;
pub(super) const CONTEXT_STORE_LINES: usize = 80;
pub(super) const CONTEXT_LINE_CHARS: usize = 240;

#[derive(Clone)]
pub(super) struct ContextLine {
    pub(super) id: String,
    pub(super) sender: String,
    pub(super) text: String,
    pub(super) timestamp: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiContextLine {
    pub id: String,
    pub sender: String,
    pub text: String,
    #[serde(default)]
    pub timestamp: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct StoredAi {
    #[serde(default)]
    pub(super) enabled: bool,
    #[serde(default = "default_base")]
    pub(super) base_url: String,
    #[serde(default = "default_model")]
    pub(super) model: String,
    /// Optional model used when a photo arrives.
    #[serde(default = "default_vision_model")]
    pub(super) vision_model: String,
    #[serde(default = "default_prompt")]
    pub(super) system_prompt: String,
    /// Require `/ai` before replying to a saved contact. When off (and AI mode
    /// is on), every message gets an instant reply.
    #[serde(default = "default_require_trigger")]
    pub(super) require_trigger: bool,
    /// `nonce.ciphertext`, both base64.
    #[serde(default)]
    pub(super) key_enc: Option<String>,
}

impl Default for StoredAi {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: default_base(),
            model: default_model(),
            vision_model: default_vision_model(),
            system_prompt: default_prompt(),
            require_trigger: false,
            key_enc: None,
        }
    }
}

/// What the webview is allowed to see (never the plaintext key).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettingsView {
    enabled: bool,
    base_url: String,
    model: String,
    vision_model: String,
    system_prompt: String,
    require_trigger: bool,
    has_key: bool,
    key_hint: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettingsInput {
    pub enabled: bool,
    pub base_url: String,
    pub model: String,
    pub vision_model: String,
    pub system_prompt: String,
    pub require_trigger: bool,
    #[serde(default)]
    pub api_key: Option<String>,
}

pub struct AiState {
    pub(super) stored: Mutex<StoredAi>,
    pub(super) api_key: Mutex<Option<String>>,
    pub(super) key: [u8; 32],
    path: PathBuf,
    pub(super) session_id: String,
    pub(super) history: Mutex<HashMap<String, Vec<(String, String)>>>,
    pub(super) last_reply: Mutex<HashMap<String, Instant>>,
    pub(super) last_reaction: Mutex<HashMap<String, Instant>>,
    pub(super) last_sticker: Mutex<HashMap<String, Instant>>,
    pub(super) last_emoji: Mutex<HashMap<String, Instant>>,
    last_global_send: Mutex<Option<Instant>>,
    pub(super) stickers: Mutex<Vec<String>>,
    context: Mutex<HashMap<String, Vec<ContextLine>>>,
}

impl AiState {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("ai.json");
        let key_path = dir.join("ai.key");
        let key = load_or_create_key(&key_path).unwrap_or_else(|err| {
            tracing::warn!(%err, "AI key file unavailable; AI replies disabled");
            [0u8; 32]
        });

        let mut stored: StoredAi = std::fs::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        // Migrate the old English default, the "reply only in indonesian"
        // workaround and the earlier formal Indonesian prompt to the casual
        // Indonesian-netizen default.
        let legacy = stored.system_prompt.trim().eq(LEGACY_PROMPT)
            || stored.system_prompt.trim().eq(LEGACY_PROMPT_ID)
            || stored.system_prompt.trim().eq(LEGACY_PROMPT_ID2)
            || stored.system_prompt.trim().eq(LEGACY_PROMPT_ID3)
            || stored.system_prompt.trim().eq(LEGACY_PROMPT_ID4)
            || stored
                .system_prompt
                .to_ascii_lowercase()
                .contains("reply only in indonesian")
            // Earlier revisions of our own default prompt are upgraded in
            // place; a fully custom prompt is left untouched.
            || (stored.system_prompt.trim().starts_with("Kamu itu gw:")
                && (!stored.system_prompt.contains("WAJIB dalam bahasa Indonesia")
                    || !stored.system_prompt.contains("tag balik nomor")
                    || !stored.system_prompt.contains("wkwk\" atau \"yaudah")
                    || !stored.system_prompt.contains("system prompt")));
        if legacy || stored.system_prompt.trim().is_empty() {
            stored.system_prompt = default_prompt();
        }

        // That model never existed; empty means "use the chat model", which is
        // multimodal on OpenCode Go.
        if stored.vision_model.trim() == "deepseek-v4-flash-vision-exp" {
            stored.vision_model.clear();
        }

        let api_key = stored
            .key_enc
            .as_deref()
            .and_then(|enc| decrypt(&key, enc).ok());

        let state = Self {
            stored: Mutex::new(stored.clone()),
            api_key: Mutex::new(api_key),
            key,
            path,
            session_id: new_session_id(),
            history: Mutex::new(HashMap::new()),
            last_reply: Mutex::new(HashMap::new()),
            last_reaction: Mutex::new(HashMap::new()),
            last_sticker: Mutex::new(HashMap::new()),
            last_emoji: Mutex::new(HashMap::new()),
            last_global_send: Mutex::new(None),
            stickers: Mutex::new(Vec::new()),
            context: Mutex::new(HashMap::new()),
        };
        let _ = state.persist(&stored);
        state
    }

    pub(super) fn view(&self) -> AiSettingsView {
        let stored = self.stored.lock().expect("ai settings lock").clone();
        let key = self.api_key.lock().expect("ai key lock");
        AiSettingsView {
            enabled: stored.enabled,
            base_url: stored.base_url,
            model: stored.model,
            vision_model: stored.vision_model,
            system_prompt: stored.system_prompt,
            require_trigger: stored.require_trigger,
            has_key: key.as_deref().is_some_and(|k| !k.is_empty()),
            key_hint: key
                .as_deref()
                .filter(|k| k.len() >= 4)
                .map(|k| format!("••••{}", &k[k.len() - 4..]))
                .unwrap_or_default(),
        }
    }

    pub(super) fn persist(&self, stored: &StoredAi) -> Result<(), String> {
        let json = serde_json::to_string_pretty(stored).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| format!("cannot save AI settings: {e}"))
    }

    pub(super) fn config(&self) -> (StoredAi, Option<String>) {
        (
            self.stored.lock().expect("ai settings lock").clone(),
            self.api_key.lock().expect("ai key lock").clone(),
        )
    }

    pub(super) async fn wait_for_send_slot(&self) {
        loop {
            let wait = {
                let mut last = self.last_global_send.lock().expect("ai send lock");
                match *last {
                    Some(previous) if previous.elapsed() < GLOBAL_SEND_GAP => {
                        Some(GLOBAL_SEND_GAP - previous.elapsed())
                    }
                    _ => {
                        *last = Some(Instant::now());
                        None
                    }
                }
            };
            match wait {
                Some(wait) => tokio::time::sleep(wait).await,
                None => break,
            }
        }
    }

    fn insert_context(&self, chat: &str, line: ContextLine) {
        let mut all = self.context.lock().expect("ai context lock");
        let lines = all.entry(chat.to_owned()).or_default();
        if lines.iter().any(|existing| existing.id == line.id) {
            return;
        }
        lines.push(line);
        lines.sort_by_key(|line| line.timestamp);
        let cutoff = now_ms() - CONTEXT_AGE_MS;
        lines.retain(|line| line.timestamp >= cutoff);
        let excess = lines.len().saturating_sub(CONTEXT_STORE_LINES);
        if excess > 0 {
            lines.drain(0..excess);
        }
    }

    pub(super) fn record_context(&self, message: &wa_core::MessageView) {
        if let Some(line) = context_line_from_message(message) {
            self.insert_context(&message.chat, line);
        }
    }

    pub(super) fn seed_context(&self, chat: &str, lines: Vec<AiContextLine>) {
        for line in lines {
            let text = line.text.trim();
            let sender = line.sender.trim();
            if text.is_empty() || sender.is_empty() {
                continue;
            }
            self.insert_context(
                chat,
                ContextLine {
                    id: line.id,
                    sender: sender.to_owned(),
                    text: clip_context_text(text),
                    timestamp: line.timestamp,
                },
            );
        }
    }

    /// A compact, bounded transcript for the model. The current message is
    /// excluded because it is sent separately as the user turn.
    pub(super) fn context_block(&self, chat: &str, current_id: &str) -> Option<String> {
        let lines = {
            let all = self.context.lock().expect("ai context lock");
            all.get(chat).cloned().unwrap_or_default()
        };
        render_context(&lines, current_id, now_ms())
    }
}

pub(super) fn clip_context_text(text: &str) -> String {
    let mut clipped: String = text.chars().take(CONTEXT_LINE_CHARS).collect();
    if text.chars().count() > CONTEXT_LINE_CHARS {
        clipped.push('…');
    }
    clipped
}

fn context_line_from_message(message: &wa_core::MessageView) -> Option<ContextLine> {
    if !matches!(
        message.kind.as_str(),
        "text" | "image" | "video" | "document" | "sticker"
    ) {
        return None;
    }
    let raw = message.text.trim();
    let text = if raw.is_empty() {
        format!("[{}]", media_label(&message.kind))
    } else {
        clip_context_text(raw)
    };
    if text.is_empty() {
        return None;
    }

    let sender = if message.from_me {
        "Gw".to_owned()
    } else {
        message
            .sender_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                message
                    .sender
                    .as_deref()
                    .map(user_part)
                    .filter(|user| !user.is_empty())
            })
            .unwrap_or_else(|| "Mereka".to_owned())
    };

    Some(ContextLine {
        id: message.id.clone(),
        sender,
        text,
        timestamp: message.timestamp,
    })
}

pub(super) fn render_context(lines: &[ContextLine], current_id: &str, now: i64) -> Option<String> {
    if lines.is_empty() {
        return None;
    }

    let cutoff = now - CONTEXT_AGE_MS;
    let mut selected = Vec::new();
    let mut used = 0usize;
    for line in lines.iter().rev() {
        if line.id == current_id || line.timestamp < cutoff {
            continue;
        }
        let piece = format!("{}: {}", line.sender, line.text);
        if used + piece.len() > CONTEXT_CHARS && !selected.is_empty() {
            break;
        }
        used += piece.len() + 1;
        selected.push(piece);
        if selected.len() >= CONTEXT_LINES {
            break;
        }
    }
    if selected.is_empty() {
        return None;
    }
    selected.reverse();
    Some(selected.join("\n"))
}

pub(super) fn default_base() -> String {
    DEFAULT_BASE.to_owned()
}

pub(super) fn default_model() -> String {
    DEFAULT_MODEL.to_owned()
}

fn default_vision_model() -> String {
    // Empty means "use the chat model". The default chat model accepts images.
    String::new()
}

fn default_require_trigger() -> bool {
    // Instant replies by default; turn the toggle on to require /ai.
    false
}

pub(super) fn load_or_create_key(path: &Path) -> std::io::Result<[u8; 32]> {
    if let Ok(bytes) = std::fs::read(path)
        && bytes.len() == 32
    {
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        return Ok(key);
    }

    let mut key = [0u8; 32];
    use aes_gcm::aead::rand_core::RngCore as _;
    OsRng.fill_bytes(&mut key);
    std::fs::write(path, key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(key)
}

pub(super) fn encrypt(key: &[u8; 32], plaintext: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| format!("cannot encrypt API key: {e}"))?;
    Ok(format!(
        "{}.{}",
        B64.encode(nonce.as_slice()),
        B64.encode(ciphertext)
    ))
}

pub(super) fn decrypt(key: &[u8; 32], encoded: &str) -> Result<String, String> {
    let (nonce, ciphertext) = encoded.split_once('.').ok_or("malformed encrypted key")?;
    let nonce = B64.decode(nonce).map_err(|e| e.to_string())?;
    let ciphertext = B64.decode(ciphertext).map_err(|e| e.to_string())?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| "could not decrypt the stored API key".to_string())?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}
