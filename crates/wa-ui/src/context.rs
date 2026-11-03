//! Bounded AI context and sticker selection, plus the media-hydration limits
//! (download timeouts, cache budgets, poster sampling).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContextMessage {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub text: String,
    /// `text`, `image`, `video`, …
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub from_me: bool,
    #[serde(default)]
    pub sender_name: String,
    #[serde(default)]
    pub sender_user: String,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub deleted_for_me: bool,
    #[serde(default)]
    pub cleared: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ContextLine {
    pub id: String,
    pub sender: String,
    pub text: String,
    pub timestamp: i64,
}

fn media_label(kind: &str) -> Option<&'static str> {
    match kind {
        "image" => Some("Photo"),
        "video" => Some("Video"),
        "audio" => Some("Audio"),
        "document" => Some("Document"),
        "sticker" => Some("Sticker"),
        _ => None,
    }
}

/// The short transcript the AI gets: visible messages only, newest `limit`, with
/// `Gw` for our own turns and a `[Photo]` style placeholder for media.
pub fn ai_context(messages: &[ContextMessage], limit: usize) -> Vec<ContextLine> {
    let mut visible: Vec<&ContextMessage> = messages
        .iter()
        .filter(|message| !(message.deleted || message.deleted_for_me || message.cleared))
        .collect();
    if visible.len() > limit {
        visible.drain(..visible.len() - limit);
    }
    visible
        .into_iter()
        .filter_map(|message| {
            let text = message.text.trim();
            let body = if !text.is_empty() {
                text.to_string()
            } else if message.kind != "text" {
                format!("[{}]", media_label(&message.kind).unwrap_or("Media"))
            } else {
                String::new()
            };
            if message.id.is_empty() || body.is_empty() {
                return None;
            }
            let sender = if message.from_me {
                "Gw".to_string()
            } else if !message.sender_name.is_empty() {
                message.sender_name.clone()
            } else if !message.sender_user.is_empty() {
                message.sender_user.clone()
            } else {
                "Mereka".to_string()
            };
            Some(ContextLine {
                id: message.id.clone(),
                sender,
                text: body,
                timestamp: message.timestamp,
            })
        })
        .collect()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StickerCandidate {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub pending: bool,
}

/// Recent stickers from the local cache: one entry per id (the newest copy),
/// no optimistic `local-` bubbles, newest first, capped.
pub fn recent_stickers(candidates: &[StickerCandidate], limit: usize) -> Vec<String> {
    let mut kept: Vec<&StickerCandidate> = Vec::new();
    for candidate in candidates {
        if candidate.id.is_empty() || candidate.pending || candidate.id.starts_with("local-") {
            continue;
        }
        match kept.iter_mut().find(|entry| entry.id == candidate.id) {
            Some(existing) => {
                if existing.timestamp < candidate.timestamp {
                    *existing = candidate;
                }
            }
            None => kept.push(candidate),
        }
    }
    kept.sort_by_key(|candidate| std::cmp::Reverse(candidate.timestamp));
    kept.into_iter().take(limit).map(|c| c.id.clone()).collect()
}

/// Only stickers whose media is cached are offered to the AI.
pub fn ai_sticker_ids(ids: &[String], ready: &[String], limit: usize) -> Vec<String> {
    let ready: std::collections::HashSet<&str> = ready.iter().map(String::as_str).collect();
    ids.iter()
        .filter(|id| ready.contains(id.as_str()))
        .take(limit)
        .cloned()
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaLimits {
    pub max_thumbs: usize,
    pub max_video_posters: usize,
    pub load_timeout_ms: i64,
    pub sticker_timeout_ms: i64,
    pub root_margin: String,
    pub poster_width: u32,
    pub poster_quality: f64,
    pub poster_timeout_ms: i64,
    pub duration_probe_ms: i64,
}

pub const MEDIA_LOAD_TIMEOUT_MS: i64 = 12_000;
pub const STICKER_LOAD_TIMEOUT_MS: i64 = 1_200;
pub const MAX_THUMBS: usize = 240;
pub const MAX_VIDEO_POSTERS: usize = 120;
pub const POSTER_WIDTH: u32 = 480;
pub const POSTER_QUALITY: f64 = 0.72;
pub const POSTER_TIMEOUT_MS: i64 = 12_000;

pub fn media_limits() -> MediaLimits {
    MediaLimits {
        max_thumbs: MAX_THUMBS,
        max_video_posters: MAX_VIDEO_POSTERS,
        load_timeout_ms: MEDIA_LOAD_TIMEOUT_MS,
        sticker_timeout_ms: STICKER_LOAD_TIMEOUT_MS,
        root_margin: "300px 0px".to_string(),
        poster_width: POSTER_WIDTH,
        poster_quality: POSTER_QUALITY,
        poster_timeout_ms: POSTER_TIMEOUT_MS,
        duration_probe_ms: 4_000,
    }
}

/// Where to seek when sampling a poster frame: 8% into the clip, clamped.
pub fn poster_seek_seconds(duration: f64) -> f64 {
    let duration = if duration.is_finite() && duration > 0.0 {
        duration
    } else {
        0.0
    };
    (duration * 0.08).clamp(0.05, 1.5)
}

/// The canvas height for a poster at `width`, defaulting to a 16:9-ish frame.
pub fn poster_height(width: u32, video_width: f64, video_height: f64) -> u32 {
    if !(video_width.is_finite() && video_width > 0.0)
        || !(video_height.is_finite() && video_height > 0.0)
    {
        return 180;
    }
    (((video_height / video_width) * width as f64).round() as u32).max(1)
}

/// A download only starts when nothing is in flight or cached for the id.
pub fn should_start(status: &str) -> bool {
    status != "loading"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: &str, kind: &str, text: &str, from_me: bool, ts: i64) -> ContextMessage {
        ContextMessage {
            id: id.into(),
            kind: kind.into(),
            text: text.into(),
            from_me,
            timestamp: ts,
            ..Default::default()
        }
    }

    #[test]
    fn context_is_bounded_and_labelled() {
        let mut messages: Vec<ContextMessage> = (0..30)
            .map(|i| {
                message(
                    &format!("m{i}"),
                    "text",
                    &format!("line {i}"),
                    i % 2 == 0,
                    i,
                )
            })
            .collect();
        let mut image = message("img", "image", "", false, 100);
        image.sender_name = "Budi".into();
        messages.push(image);
        messages.push(ContextMessage {
            id: "gone".into(),
            text: "deleted".into(),
            deleted: true,
            ..Default::default()
        });

        let lines = ai_context(&messages, 24);
        assert_eq!(lines.len(), 24);
        assert_eq!(lines.last().unwrap().id, "img");
        assert_eq!(lines.last().unwrap().text, "[Photo]");
        assert_eq!(lines.last().unwrap().sender, "Budi");
        assert!(lines.iter().all(|line| line.id != "gone"));
        assert!(
            lines.iter().any(|line| line.sender == "Gw"),
            "own turns are Gw"
        );
        assert!(
            lines.iter().any(|line| line.sender == "Mereka"),
            "others fall back"
        );
        assert_eq!(ai_context(&[], 24).len(), 0);
    }

    #[test]
    fn context_skips_media_without_a_label_and_empty_text() {
        let lines = ai_context(
            &[
                message("a", "text", "   ", false, 1),
                message("b", "other", "", false, 2),
                ContextMessage {
                    id: "c".into(),
                    kind: "text".into(),
                    text: "hello".into(),
                    ..Default::default()
                },
            ],
            24,
        );
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].id, "b");
        assert_eq!(lines[0].text, "[Media]");
        assert_eq!(lines[1].id, "c");
        assert_eq!(lines[1].sender, "Mereka");
    }

    #[test]
    fn recent_stickers_dedupe_and_sort() {
        let candidates = vec![
            StickerCandidate {
                id: "a".into(),
                timestamp: 10,
                pending: false,
            },
            StickerCandidate {
                id: "a".into(),
                timestamp: 30,
                pending: false,
            },
            StickerCandidate {
                id: "local-1".into(),
                timestamp: 40,
                pending: false,
            },
            StickerCandidate {
                id: "b".into(),
                timestamp: 20,
                pending: true,
            },
            StickerCandidate {
                id: "c".into(),
                timestamp: 5,
                pending: false,
            },
        ];
        assert_eq!(
            recent_stickers(&candidates, 80),
            vec!["a".to_string(), "c".to_string()]
        );
        assert_eq!(recent_stickers(&candidates, 1), vec!["a".to_string()]);
    }

    #[test]
    fn ai_stickers_need_cached_media() {
        let ids = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let ready = vec!["b".to_string(), "c".to_string()];
        assert_eq!(
            ai_sticker_ids(&ids, &ready, 80),
            vec!["b".to_string(), "c".to_string()]
        );
        assert_eq!(ai_sticker_ids(&ids, &ready, 1), vec!["b".to_string()]);
    }

    #[test]
    fn media_limits_and_poster_math() {
        let limits = media_limits();
        assert_eq!(limits.max_thumbs, 240);
        assert_eq!(limits.max_video_posters, 120);
        assert_eq!(limits.load_timeout_ms, 12_000);
        assert_eq!(limits.sticker_timeout_ms, 1_200);
        assert_eq!(limits.poster_width, 480);
        assert_eq!(limits.poster_quality, 0.72);
        assert_eq!(limits.root_margin, "300px 0px");

        assert_eq!(poster_seek_seconds(10.0), 0.8);
        assert_eq!(poster_seek_seconds(100.0), 1.5);
        assert_eq!(poster_seek_seconds(0.0), 0.05);
        assert_eq!(poster_seek_seconds(f64::NAN), 0.05);

        assert_eq!(poster_height(480, 1920.0, 1080.0), 270);
        assert_eq!(poster_height(480, 0.0, 0.0), 180);
        assert!(should_start("ready"));
        assert!(!should_start("loading"));
    }
}
