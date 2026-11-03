//! On-demand history requests: the anchor rule (page behind the oldest local
//! message) and the order chats are re-synced in.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HistoryChat {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub is_channel: bool,
    #[serde(default)]
    pub message_count: usize,
    #[serde(default)]
    pub last_ts: i64,
    #[serde(default)]
    pub messages: Vec<HistoryMessage>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct HistoryMessage {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub from_me: bool,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnchorRequest {
    pub chat: String,
    pub oldest_id: String,
    pub oldest_from_me: bool,
    pub oldest_timestamp: i64,
    pub count: i64,
}

/// A pending local/AI bubble is not an anchor: the phone has no such message.
fn is_local(id: &str) -> bool {
    id.starts_with("local-") || id.starts_with("ai-")
}

/// Page behind the oldest locally stored message; an empty chat asks for the
/// latest window (no anchor).
pub fn anchor_request(chat: &HistoryChat) -> Option<AnchorRequest> {
    if chat.jid.is_empty() {
        return None;
    }
    let anchor = chat
        .messages
        .iter()
        .find(|message| !message.deleted && !message.id.is_empty() && !is_local(&message.id));
    Some(AnchorRequest {
        chat: chat.jid.clone(),
        oldest_id: anchor.map(|message| message.id.clone()).unwrap_or_default(),
        oldest_from_me: anchor.map(|message| message.from_me).unwrap_or(false),
        oldest_timestamp: anchor.map(|message| message.timestamp).unwrap_or(0),
        count: 50,
    })
}

/// Chats that never produced a bubble go first, then the most recently active.
pub fn sync_targets(chats: &[HistoryChat], limit: usize) -> Vec<String> {
    let mut eligible: Vec<&HistoryChat> = chats
        .iter()
        .filter(|chat| !chat.jid.is_empty() && !chat.deleted && !chat.is_channel)
        .collect();
    eligible.sort_by(|a, b| {
        let a_empty = if a.message_count == 0 { 0 } else { 1 };
        let b_empty = if b.message_count == 0 { 0 } else { 1 };
        a_empty
            .cmp(&b_empty)
            .then_with(|| b.last_ts.cmp(&a.last_ts))
    });
    eligible
        .into_iter()
        .take(limit)
        .map(|chat| chat.jid.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(jid: &str, count: usize, last_ts: i64) -> HistoryChat {
        HistoryChat {
            jid: jid.into(),
            message_count: count,
            last_ts,
            ..Default::default()
        }
    }

    #[test]
    fn anchors_on_the_oldest_real_message() {
        let req = anchor_request(&HistoryChat {
            jid: "1@s.whatsapp.net".into(),
            messages: vec![
                HistoryMessage {
                    id: "local-1".into(),
                    timestamp: 10,
                    ..Default::default()
                },
                HistoryMessage {
                    id: "m1".into(),
                    timestamp: 20,
                    from_me: true,
                    ..Default::default()
                },
                HistoryMessage {
                    id: "m2".into(),
                    timestamp: 30,
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .unwrap();
        assert_eq!(req.oldest_id, "m1");
        assert!(req.oldest_from_me);
        assert_eq!(req.oldest_timestamp, 20);
        assert_eq!(req.count, 50);
    }

    #[test]
    fn deleted_and_local_messages_are_skipped() {
        let req = anchor_request(&HistoryChat {
            jid: "1@s.whatsapp.net".into(),
            messages: vec![
                HistoryMessage {
                    id: "m0".into(),
                    timestamp: 5,
                    deleted: true,
                    ..Default::default()
                },
                HistoryMessage {
                    id: "ai-9".into(),
                    timestamp: 6,
                    ..Default::default()
                },
                HistoryMessage {
                    id: "m1".into(),
                    timestamp: 20,
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .unwrap();
        assert_eq!(req.oldest_id, "m1");
    }

    #[test]
    fn empty_chats_have_no_anchor() {
        let req = anchor_request(&chat("1@s.whatsapp.net", 0, 0)).unwrap();
        assert_eq!(req.oldest_id, "");
        assert_eq!(req.oldest_timestamp, 0);
        assert!(anchor_request(&HistoryChat::default()).is_none());
    }

    #[test]
    fn sync_targets_put_empty_chats_first() {
        let chats = vec![
            chat("old@s.whatsapp.net", 5, 100),
            chat("new@s.whatsapp.net", 5, 300),
            chat("empty@s.whatsapp.net", 0, 50),
            chat("gone@s.whatsapp.net", 0, 900),
            chat("chan@newsletter", 0, 1000),
        ];
        let mut chats = chats;
        chats[3].deleted = true;
        chats[4].is_channel = true;
        let targets = sync_targets(&chats, 10);
        assert_eq!(
            targets,
            vec![
                "empty@s.whatsapp.net".to_string(),
                "new@s.whatsapp.net".to_string(),
                "old@s.whatsapp.net".to_string()
            ]
        );
        assert_eq!(sync_targets(&chats, 2).len(), 2);
    }
}
