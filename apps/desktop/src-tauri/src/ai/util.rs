//! Clock, randomness and small text helpers shared by the AI modules.

use std::time::{SystemTime, UNIX_EPOCH};

const MIN_REPLY_DELAY_MS: u64 = 1_800;
const MAX_REPLY_DELAY_MS: u64 = 5_500;

/// Wall clock in milliseconds; `0` when the clock is before the Unix epoch.
pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// Uniform value in `0..upper`, seeded from the OS RNG (no extra dependency).
pub(super) fn random_below(upper: usize) -> usize {
    if upper <= 1 {
        return 0;
    }
    use aes_gcm::aead::rand_core::RngCore as _;
    let mut bytes = [0u8; 8];
    aes_gcm::aead::OsRng.fill_bytes(&mut bytes);
    (u64::from_le_bytes(bytes) % upper as u64) as usize
}

/// Human-like pause before sending: a small base delay plus typing time.
pub(super) fn human_delay_ms(reply: &str, fast: bool) -> u64 {
    let (min, max, per_char, cap) = if fast {
        (300u64, 900u64, 20u64, 800u64)
    } else {
        (MIN_REPLY_DELAY_MS, MAX_REPLY_DELAY_MS, 160u64, 7_000u64)
    };
    let range = max.saturating_sub(min).max(1) as usize;
    let typing = (reply.chars().count() as u64 * per_char).min(cap);
    let jitter = random_below(400) as u64;
    min + random_below(range) as u64 + typing + jitter
}

pub(super) fn is_emoji(ch: char) -> bool {
    let code = ch as u32;
    (0x1F300..=0x1FAFF).contains(&code)
        || (0x2600..=0x27BF).contains(&code)
        || (0xFE00..=0xFE0F).contains(&code)
        || (0x1F1E6..=0x1F1FF).contains(&code)
        || code == 0x200D
}

pub(super) fn contains_emoji(text: &str) -> bool {
    text.chars().any(is_emoji)
}

pub(super) fn strip_emoji(text: &str) -> String {
    let cleaned: String = text.chars().filter(|ch| !is_emoji(*ch)).collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn media_label(kind: &str) -> &'static str {
    match kind {
        "image" => "Foto",
        "video" => "Video/GIF",
        "document" => "Dokumen",
        "sticker" => "Stiker",
        _ => "Media",
    }
}

/// Extracts the bare user part of a JID (`62812...:53@s.whatsapp.net` -> `62812...`).
pub(super) fn user_part(jid: &str) -> String {
    jid.split('@')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_owned()
}

/// Random id sent as `x-opencode-session`, so each app run is a new session.
pub(super) fn new_session_id() -> String {
    use aes_gcm::aead::rand_core::RngCore as _;
    let mut bytes = [0u8; 8];
    aes_gcm::aead::OsRng.fill_bytes(&mut bytes);
    let mut id = String::from("wa-desk-");
    for byte in bytes {
        id.push_str(&format!("{byte:02x}"));
    }
    id
}
