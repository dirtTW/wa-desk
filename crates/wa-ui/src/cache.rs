//! The webview's local cache: the change fingerprint, the payload written to
//! localStorage, the restore shape and the batched profile-photo queue.
//!
//! Messages are carried as opaque JSON so every field a build adds survives the
//! round-trip; only the shapes this module owns (ordering, truncation, stub
//! filtering, thumbnail stripping) are rewritten.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FingerMessage {
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub text_len: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub edited: bool,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub reactions: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FingerChat {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub unread: i64,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub cleared: bool,
    #[serde(default)]
    pub messages: Vec<FingerMessage>,
}

/// A cheap fingerprint of everything persisted, so a no-op save can return
/// before re-serialising ~1.8 MB of JSON.
pub fn fingerprint(chats: &[FingerChat], active: &str, photos: usize, starred: usize) -> String {
    let mut hash: i32 = 0;
    let mut message_count = 0usize;
    for chat in chats {
        let mut acc: i32 = 0;
        for message in &chat.messages {
            let status = message
                .status
                .chars()
                .next()
                .map(|ch| ch as i32)
                .unwrap_or(0);
            acc = acc
                .wrapping_mul(31)
                .wrapping_add(message.timestamp as i32)
                .wrapping_add(message.text_len as i32)
                .wrapping_add(status)
                .wrapping_add(if message.edited { 1 } else { 0 })
                .wrapping_add(if message.deleted { 1 } else { 0 })
                .wrapping_add(message.reactions as i32);
        }
        message_count += chat.messages.len();
        hash = hash
            .wrapping_mul(31)
            .wrapping_add(acc)
            .wrapping_add(chat.unread as i32)
            .wrapping_add(chat.name.chars().count() as i32)
            .wrapping_add(if chat.deleted { 1 } else { 0 })
            .wrapping_add(if chat.cleared { 1 } else { 0 });
    }
    format!(
        "{}:{}:{}:{}:{}:{}",
        chats.len(),
        message_count,
        active,
        photos,
        starred,
        hash
    )
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CacheChatInput {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub last_ts: i64,
    #[serde(default)]
    pub unread: i64,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub cleared: bool,
    #[serde(default)]
    pub messages: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CachePayloadInput {
    #[serde(default)]
    pub chats: Vec<CacheChatInput>,
    #[serde(default)]
    pub active: String,
    #[serde(default)]
    pub group_meta: Value,
    #[serde(default)]
    pub starred: Vec<String>,
    #[serde(default)]
    pub photos: Vec<(String, String)>,
    #[serde(default = "default_max_chats")]
    pub max_chats: usize,
    #[serde(default = "default_max_messages")]
    pub max_messages_per_chat: usize,
}

fn default_max_chats() -> usize {
    200
}

fn default_max_messages() -> usize {
    300
}

/// Strip a message's `link.thumb` (a large data URI that is rebuilt on demand).
fn strip_thumb(message: &Value) -> Value {
    let Some(object) = message.as_object() else {
        return message.clone();
    };
    let Some(link) = object.get("link").and_then(Value::as_object) else {
        return message.clone();
    };
    if !link.contains_key("thumb") {
        return message.clone();
    }
    let mut link = link.clone();
    link.insert("thumb".to_string(), Value::Null);
    let mut object = object.clone();
    object.insert("link".to_string(), Value::Object(link));
    Value::Object(object)
}

/// Build the JSON text stored under the cache key.
pub fn payload(input: &CachePayloadInput) -> String {
    let mut chats: Vec<&CacheChatInput> = input.chats.iter().collect();
    chats.sort_by_key(|chat| std::cmp::Reverse(chat.last_ts));
    chats.truncate(input.max_chats);

    let chats: Vec<Value> = chats
        .iter()
        .map(|chat| {
            let skip = chat
                .messages
                .len()
                .saturating_sub(input.max_messages_per_chat);
            let messages: Vec<Value> = chat.messages[skip..].iter().map(strip_thumb).collect();
            json!({
                "jid": chat.jid,
                "name": chat.name,
                "isGroup": chat.is_group,
                "lastTs": chat.last_ts,
                "unread": chat.unread,
                "deleted": chat.deleted,
                "cleared": chat.cleared,
                "messages": messages,
            })
        })
        .collect();

    let photos: Map<String, Value> = input
        .photos
        .iter()
        .filter(|(_, url)| url.starts_with("http"))
        .map(|(jid, url)| (jid.clone(), Value::String(url.clone())))
        .collect();

    json!({
        "v": 1,
        "chats": chats,
        "active": input.active,
        "groupMeta": input.group_meta,
        "starred": input.starred,
        "photos": photos,
    })
    .to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoredChat {
    pub jid: String,
    pub name: String,
    pub is_group: bool,
    pub messages: Vec<Value>,
    pub last_ts: i64,
    pub unread: i64,
    pub deleted: bool,
    pub cleared: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreOutput {
    pub chats: Vec<RestoredChat>,
    pub group_meta: Value,
    pub starred: Vec<String>,
    pub photos: Vec<(String, String)>,
}

fn stub_message(message: &Value) -> bool {
    let object = message.as_object();
    let text = object
        .and_then(|value| value.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if !text.is_empty() {
        return false;
    }
    // Protocol/notification stubs written by older builds have no media type.
    match object
        .and_then(|value| value.get("kind"))
        .and_then(Value::as_str)
    {
        Some(kind) => kind.is_empty() || kind == "other",
        None => true,
    }
}

/// Parse a stored cache payload into the shape the webview restores.
pub fn restore(raw: &str) -> RestoreOutput {
    let parsed: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
    let mut output = RestoreOutput {
        chats: Vec::new(),
        group_meta: json!({}),
        starred: Vec::new(),
        photos: Vec::new(),
    };
    let Some(object) = parsed.as_object() else {
        return output;
    };

    if let Some(meta) = object.get("groupMeta").filter(|value| value.is_object()) {
        output.group_meta = meta.clone();
    }
    if let Some(starred) = object.get("starred").and_then(Value::as_array) {
        output.starred = starred
            .iter()
            .filter_map(Value::as_str)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .collect();
    }
    if let Some(photos) = object.get("photos").and_then(Value::as_object) {
        output.photos = photos
            .iter()
            .filter_map(|(jid, url)| {
                url.as_str()
                    .filter(|url| url.starts_with("http"))
                    .map(|url| (jid.clone(), url.to_string()))
            })
            .collect();
    }

    let Some(chats) = object.get("chats").and_then(Value::as_array) else {
        return output;
    };
    for chat in chats {
        let Some(chat) = chat.as_object() else {
            continue;
        };
        let jid = chat
            .get("jid")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if jid.is_empty() || jid.ends_with("@broadcast") {
            continue;
        }
        let messages: Vec<Value> = chat
            .get("messages")
            .and_then(Value::as_array)
            .map(|messages| {
                messages
                    .iter()
                    .filter(|message| !stub_message(message))
                    .map(|message| {
                        let mut message = message.clone();
                        if let Some(object) = message.as_object_mut() {
                            object.insert("pending".to_string(), Value::Bool(false));
                        }
                        message
                    })
                    .collect()
            })
            .unwrap_or_default();
        let fallback_name = jid.split('@').next().unwrap_or("").to_string();
        let name = chat
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .unwrap_or(fallback_name);
        output.chats.push(RestoredChat {
            jid,
            name,
            is_group: chat
                .get("isGroup")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            messages,
            last_ts: chat.get("lastTs").and_then(Value::as_i64).unwrap_or(0),
            unread: chat.get("unread").and_then(Value::as_i64).unwrap_or(0),
            deleted: chat
                .get("deleted")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            cleared: chat
                .get("cleared")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    output
}

/// Filter the photo candidates down to the ones worth requesting.
pub fn photo_queue(
    candidates: &[String],
    connected: bool,
    have: &[String],
    failed: &[String],
    queued: &[String],
) -> Vec<String> {
    let have: HashSet<&str> = have.iter().map(String::as_str).collect();
    let failed: HashSet<&str> = failed.iter().map(String::as_str).collect();
    let queued: HashSet<&str> = queued.iter().map(String::as_str).collect();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for jid in candidates {
        if !seen.insert(jid.as_str()) {
            continue;
        }
        if crate::state::photo_queue_allowed(
            jid,
            connected,
            have.contains(jid.as_str()),
            failed.contains(jid.as_str()),
            queued.contains(jid.as_str()),
        ) {
            out.push(jid.clone());
        }
    }
    out
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DiskChatInput {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub last_ts: i64,
    #[serde(default)]
    pub unread: i64,
    #[serde(default)]
    pub last_text: String,
}

/// The disk mirror of the chat list: metadata only, newest first, capped.
pub fn disk_payload(chats: &[DiskChatInput], limit: usize) -> String {
    let mut chats: Vec<&DiskChatInput> = chats.iter().collect();
    chats.sort_by_key(|chat| std::cmp::Reverse(chat.last_ts));
    chats.truncate(limit);
    let rows: Vec<Value> = chats
        .iter()
        .map(|chat| {
            let last_text: String = chat.last_text.chars().take(120).collect();
            json!({
                "jid": chat.jid,
                "name": chat.name,
                "isGroup": chat.is_group,
                "lastTs": chat.last_ts,
                "unread": chat.unread,
                "lastText": last_text,
            })
        })
        .collect();
    json!({ "v": 1, "chats": rows }).to_string()
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DiskChatRow {
    pub jid: String,
    pub name: String,
    pub is_group: bool,
    pub last_ts: i64,
    pub unread: i64,
    /// Synthesized preview message from the metadata mirror.
    pub last_text: Option<String>,
}

/// Parse the disk chat-list mirror into rows (broadcasts skipped, jid fallback
/// name), so a restart without a webview cache still has a sidebar.
pub fn restore_disk(raw: &str) -> Vec<DiskChatRow> {
    let parsed: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
    let Some(chats) = parsed.get("chats").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for chat in chats {
        let Some(chat) = chat.as_object() else {
            continue;
        };
        let jid = chat
            .get("jid")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if jid.is_empty() || jid.ends_with("@broadcast") {
            continue;
        }
        let fallback = jid.split('@').next().unwrap_or("").to_string();
        let name = chat
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .unwrap_or(fallback);
        let last_text = chat
            .get("lastText")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_string);
        out.push(DiskChatRow {
            jid,
            name,
            is_group: chat
                .get("isGroup")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            last_ts: chat.get("lastTs").and_then(Value::as_i64).unwrap_or(0),
            unread: chat.get("unread").and_then(Value::as_i64).unwrap_or(0),
            last_text,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_payload_orders_truncates_and_trims_the_preview() {
        let long = "x".repeat(200);
        let chats = vec![
            DiskChatInput {
                jid: "old@s.whatsapp.net".into(),
                name: "Old".into(),
                last_ts: 100,
                last_text: "old".into(),
                ..Default::default()
            },
            DiskChatInput {
                jid: "new@s.whatsapp.net".into(),
                name: "New".into(),
                is_group: true,
                last_ts: 300,
                unread: 2,
                last_text: long,
            },
        ];
        let parsed: Value = serde_json::from_str(&disk_payload(&chats, 1)).unwrap();
        let rows = parsed["chats"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["jid"], "new@s.whatsapp.net");
        assert_eq!(rows[0]["isGroup"], true);
        assert_eq!(rows[0]["lastText"].as_str().unwrap().chars().count(), 120);
        assert_eq!(parsed["v"], 1);
    }

    fn finger(ts: i64, len: i64, status: &str) -> FingerMessage {
        FingerMessage {
            timestamp: ts,
            text_len: len,
            status: status.into(),
            ..Default::default()
        }
    }

    #[test]
    fn disk_restore_shapes_rows() {
        let raw = json!({
            "v": 1,
            "chats": [
                {"jid": "a@s.whatsapp.net", "name": "Ada", "lastTs": 10, "unread": 2, "lastText": "hi"},
                {"jid": "b@g.us", "isGroup": true, "lastTs": 5},
                {"jid": "status@broadcast", "name": "S"}
            ]
        })
        .to_string();
        let rows = restore_disk(&raw);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "Ada");
        assert_eq!(rows[0].last_text.as_deref(), Some("hi"));
        assert_eq!(rows[1].name, "b", "jid fallback name");
        assert!(rows[1].is_group);
        assert!(rows[1].last_text.is_none());
        assert!(restore_disk("nope").is_empty());
        assert!(restore_disk("{}").is_empty());
    }

    #[test]
    fn fingerprints_change_with_content() {
        let mut chats = vec![FingerChat {
            name: "Ada".into(),
            unread: 1,
            messages: vec![finger(1000, 5, "read")],
            ..Default::default()
        }];
        let base = fingerprint(&chats, "a", 3, 2);
        assert!(base.starts_with("1:1:a:3:2:"));
        assert_eq!(base, fingerprint(&chats, "a", 3, 2), "stable");

        chats[0].unread = 2;
        assert_ne!(base, fingerprint(&chats, "a", 3, 2));
        chats[0].unread = 1;
        chats[0].messages.push(finger(2000, 5, "read"));
        assert_ne!(base, fingerprint(&chats, "a", 3, 2));
        assert!(fingerprint(&chats, "a", 3, 2).starts_with("1:2:"));
    }

    #[test]
    fn payload_orders_truncates_and_strips_thumbs() {
        let input = CachePayloadInput {
            chats: vec![
                CacheChatInput {
                    jid: "old@s.whatsapp.net".into(),
                    name: "Old".into(),
                    last_ts: 100,
                    messages: vec![json!({"id": "o1", "text": "hi"})],
                    ..Default::default()
                },
                CacheChatInput {
                    jid: "new@s.whatsapp.net".into(),
                    name: "New".into(),
                    last_ts: 200,
                    messages: vec![
                        json!({"id": "n1", "text": "a"}),
                        json!({"id": "n2", "text": "b", "link": {"url": "https://x", "thumb": "data:image/jpeg;base64,AA"}}),
                    ],
                    ..Default::default()
                },
            ],
            active: "new@s.whatsapp.net".into(),
            group_meta: json!({"g@g.us": {"subject": "Team"}}),
            starred: vec!["n1".into()],
            photos: vec![
                ("a@s.whatsapp.net".into(), "https://pps/x.jpg".into()),
                ("b@s.whatsapp.net".into(), "blob:stale".into()),
            ],
            max_chats: 1,
            max_messages_per_chat: 5,
        };
        let text = payload(&input);
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["v"], 1);
        let chats = parsed["chats"].as_array().unwrap();
        assert_eq!(chats.len(), 1, "max_chats");
        assert_eq!(chats[0]["jid"], "new@s.whatsapp.net");
        assert!(chats[0]["messages"][1]["link"]["thumb"].is_null());
        assert_eq!(
            parsed["photos"].as_object().unwrap().len(),
            1,
            "only http photos"
        );
        assert_eq!(parsed["starred"][0], "n1");
    }

    #[test]
    fn payload_keeps_the_newest_messages() {
        let input = CachePayloadInput {
            chats: vec![CacheChatInput {
                jid: "a@s.whatsapp.net".into(),
                name: "A".into(),
                last_ts: 1,
                messages: (0..5).map(|i| json!({"id": i, "text": "x"})).collect(),
                ..Default::default()
            }],
            active: String::new(),
            group_meta: json!({}),
            starred: Vec::new(),
            photos: Vec::new(),
            max_chats: 200,
            max_messages_per_chat: 2,
        };
        let parsed: Value = serde_json::from_str(&payload(&input)).unwrap();
        assert_eq!(parsed["chats"][0]["messages"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["chats"][0]["messages"][0]["id"], 3);
    }

    #[test]
    fn restore_drops_broadcasts_and_stubs() {
        let raw = json!({
            "v": 1,
            "chats": [
                {"jid": "status@broadcast", "messages": []},
                {"jid": "a@s.whatsapp.net", "name": "Ada", "messages": [
                    {"id": "1", "text": "hi", "pending": true},
                    {"id": "2", "kind": "other"},
                    {"id": "3", "kind": "image", "text": ""}
                ], "lastTs": 5, "unread": 2, "deleted": true, "cleared": false},
                {"jid": "b@s.whatsapp.net", "messages": []}
            ],
            "groupMeta": {"g@g.us": {"subject": "Team"}},
            "starred": ["1", ""],
            "photos": {"a@s.whatsapp.net": "https://pps/x.jpg", "b@s.whatsapp.net": "blob:bad"}
        })
        .to_string();
        let out = restore(&raw);
        assert_eq!(out.chats.len(), 2, "broadcast skipped");
        let chat = &out.chats[0];
        assert_eq!(chat.jid, "a@s.whatsapp.net");
        assert_eq!(chat.name, "Ada");
        assert_eq!(chat.messages.len(), 2, "kind:other stub dropped");
        assert_eq!(chat.messages[0]["pending"], false);
        assert!(chat.deleted && !chat.cleared);
        assert_eq!(chat.unread, 2);
        assert_eq!(out.chats[1].name, "b", "jid fallback name");
        assert_eq!(out.starred, vec!["1".to_string()]);
        assert_eq!(out.photos.len(), 1);
        assert_eq!(out.group_meta["g@g.us"]["subject"], "Team");
    }

    #[test]
    fn restore_tolerates_garbage() {
        assert!(restore("not json").chats.is_empty());
        assert!(restore("null").chats.is_empty());
        assert!(restore("{}").chats.is_empty());
    }

    #[test]
    fn photo_queue_filters_and_dedupes() {
        let candidates = vec![
            "a@s.whatsapp.net".to_string(),
            "a@s.whatsapp.net".to_string(),
            "b@s.whatsapp.net".to_string(),
            "c@s.whatsapp.net".to_string(),
            "0@s.whatsapp.net".to_string(),
            "d@s.whatsapp.net".to_string(),
        ];
        let out = photo_queue(
            &candidates,
            true,
            &["b@s.whatsapp.net".to_string()],
            &["c@s.whatsapp.net".to_string()],
            &["d@s.whatsapp.net".to_string()],
        );
        assert_eq!(out, vec!["a@s.whatsapp.net".to_string()]);
        assert!(photo_queue(&candidates, false, &[], &[], &[]).is_empty());
    }
}
