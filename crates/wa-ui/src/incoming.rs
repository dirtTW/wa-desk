//! Incoming-message application rules: duplicate/stub upgrades, own-message
//! echo folding, unread bookkeeping, receipt ordering and the AI echo guard.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExistingView {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub has_stats: bool,
    #[serde(default)]
    pub forwarded: bool,
    #[serde(default)]
    pub has_link: bool,
    #[serde(default)]
    pub has_link_thumb: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IncomingView {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub from_me: bool,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub has_stats: bool,
    #[serde(default)]
    pub forwarded: bool,
    #[serde(default)]
    pub has_link_thumb: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct EchoView {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub timestamp: i64,
}

#[derive(Debug, Default, Serialize)]
pub struct DuplicatePlan {
    /// A key-only stub gains the body (or a view-once placeholder gains media).
    pub upgrade: bool,
    pub fold_stats: bool,
    pub fold_forwarded: bool,
    pub fold_link: bool,
    pub changed: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    /// The id already exists; fold what is new into it.
    Duplicate(DuplicatePlan),
    /// One of our own messages came back: turn the optimistic bubble into it.
    EchoPending { index: usize },
    /// A new message.
    Append,
}

fn is_stub(existing: &ExistingView) -> bool {
    existing.text.trim().is_empty()
        && matches!(
            existing.kind.as_str(),
            "" | "text" | "unsupported" | "view_once"
        )
}

fn has_content(message: &IncomingView) -> bool {
    !message.text.trim().is_empty()
        || (!message.kind.is_empty() && message.kind != "text" && message.kind != "unsupported")
}

/// Decide what a message event does to a chat that already has (or lacks) the id.
pub fn classify(
    existing: Option<&ExistingView>,
    message: &IncomingView,
    echo: Option<usize>,
) -> Decision {
    if let Some(existing) = existing {
        let upgrade = is_stub(existing) && has_content(message);
        let fold_stats = message.has_stats && !existing.has_stats;
        let fold_forwarded = message.forwarded && !existing.forwarded;
        let fold_link = message.has_link_thumb && !existing.has_link_thumb;
        return Decision::Duplicate(DuplicatePlan {
            upgrade,
            fold_stats,
            fold_forwarded,
            fold_link,
            changed: upgrade || fold_stats || fold_forwarded || fold_link,
        });
    }
    if !message.from_me {
        return Decision::Append;
    }
    match echo {
        Some(index) => Decision::EchoPending { index },
        None => Decision::Append,
    }
}

/// The newest optimistic bubble an echoed own message belongs to: same kind,
/// same text for text messages, within two minutes.
pub fn pending_echo(candidates: &[EchoView], message: &IncomingView) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, candidate)| {
            let same_kind = candidate.kind == message.kind;
            let same_text = message.kind != "text" || candidate.text == message.text;
            let close = (message.timestamp - candidate.timestamp).abs() < 120_000;
            (same_kind && same_text && close).then_some(index)
        })
}

#[derive(Debug, Serialize)]
pub struct UnreadRule {
    /// The active chat clears its badge when the message lands.
    pub clear: bool,
    /// A background chat bumps its badge.
    pub increment: bool,
}

/// A message in the open chat (or one we sent from another device) is not
/// unread news; everything else is.
pub fn unread_rule(active: bool, from_me: bool) -> UnreadRule {
    if active || from_me {
        UnreadRule {
            clear: true,
            increment: false,
        }
    } else {
        UnreadRule {
            clear: false,
            increment: true,
        }
    }
}

/// Ordering for receipts: a delayed `sent` must not undo `read`.
pub fn status_rank(status: &str) -> u8 {
    match status {
        "read" | "played" => 3,
        "delivered" => 2,
        "sent" => 1,
        _ => 0,
    }
}

/// A receipt may only advance a message's state, never roll it back.
pub fn receipt_should_apply(current: &str, incoming: &str) -> bool {
    status_rank(incoming) > status_rank(current)
}

/// Everything we send in "Message yourself" is delivered immediately (the phone
/// owns that chat), so the bubble starts on the grey double tick.
pub fn self_chat_status(is_self_chat: bool) -> &'static str {
    if is_self_chat { "delivered" } else { "sent" }
}

/// The AI reply listener and the message echo race: a reply with the same text
/// within three seconds is the same message.
pub fn is_ai_echo(messages: &[(bool, String, i64)], text: &str, now: i64) -> bool {
    messages.iter().any(|(from_me, body, timestamp)| {
        *from_me && body == text && (timestamp - now).abs() < 3_000
    })
}

/// The newest optimistic bubble (`pending: true`), for a send echo.
pub fn newest_pending(flags: &[bool]) -> Option<usize> {
    flags.iter().rposition(|pending| *pending)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn incoming(kind: &str, text: &str, from_me: bool, ts: i64) -> IncomingView {
        IncomingView {
            kind: kind.into(),
            text: text.into(),
            from_me,
            timestamp: ts,
            ..Default::default()
        }
    }

    #[test]
    fn new_messages_append() {
        match classify(None, &incoming("text", "hi", false, 1), None) {
            Decision::Append => {}
            other => panic!("expected append, got {other:?}"),
        }
    }

    #[test]
    fn existing_ids_fold_what_is_new() {
        let existing = ExistingView {
            text: String::new(),
            kind: "text".into(),
            has_stats: false,
            forwarded: false,
            has_link: false,
            has_link_thumb: false,
        };
        let mut message = incoming("image", "", false, 10);
        message.has_stats = true;
        message.forwarded = true;
        message.has_link_thumb = true;
        match classify(Some(&existing), &message, None) {
            Decision::Duplicate(plan) => {
                assert!(plan.upgrade, "empty text stub gains media");
                assert!(plan.fold_stats && plan.fold_forwarded && plan.fold_link);
                assert!(plan.changed);
            }
            other => panic!("expected duplicate, got {other:?}"),
        }

        let rich = ExistingView {
            text: "hello".into(),
            kind: "text".into(),
            has_stats: true,
            forwarded: true,
            has_link_thumb: true,
            ..Default::default()
        };
        match classify(Some(&rich), &incoming("text", "hello", false, 10), None) {
            Decision::Duplicate(plan) => assert!(!plan.changed, "nothing new to fold"),
            other => panic!("expected duplicate, got {other:?}"),
        }
    }

    #[test]
    fn own_echoes_fold_into_the_pending_bubble() {
        let candidates = vec![
            EchoView {
                kind: "text".into(),
                text: "older".into(),
                timestamp: 1_000,
            },
            EchoView {
                kind: "text".into(),
                text: "hello".into(),
                timestamp: 10_000,
            },
        ];
        let message = incoming("text", "hello", true, 11_000);
        assert_eq!(pending_echo(&candidates, &message), Some(1));
        assert_eq!(
            pending_echo(&candidates, &incoming("text", "hello", true, 400_000)),
            None,
            "older than two minutes"
        );
        match classify(None, &message, Some(1)) {
            Decision::EchoPending { index } => assert_eq!(index, 1),
            other => panic!("expected echo, got {other:?}"),
        }
        // Media matches by kind only.
        let media = vec![EchoView {
            kind: "image".into(),
            text: String::new(),
            timestamp: 10_000,
        }];
        assert_eq!(
            pending_echo(&media, &incoming("image", "", true, 10_500)),
            Some(0)
        );
    }

    #[test]
    fn unread_rules() {
        assert!(unread_rule(false, false).increment);
        assert!(!unread_rule(false, false).clear);
        assert!(unread_rule(true, false).clear);
        assert!(!unread_rule(true, false).increment);
        assert!(unread_rule(false, true).clear, "own messages are not news");
    }

    #[test]
    fn receipts_only_advance() {
        assert!(receipt_should_apply("sent", "delivered"));
        assert!(receipt_should_apply("delivered", "read"));
        assert!(!receipt_should_apply("read", "sent"));
        assert!(!receipt_should_apply("read", "read"));
        assert!(!receipt_should_apply("delivered", "sent"));
        assert_eq!(self_chat_status(true), "delivered");
        assert_eq!(self_chat_status(false), "sent");
    }

    #[test]
    fn newest_pending_is_the_last_one() {
        assert_eq!(newest_pending(&[false, true, false, true]), Some(3));
        assert_eq!(newest_pending(&[false, false]), None);
        assert_eq!(newest_pending(&[]), None);
    }

    #[test]
    fn ai_echo_guard() {
        let messages = vec![
            (true, "hello".to_string(), 10_000),
            (false, "hello".to_string(), 10_000),
        ];
        assert!(is_ai_echo(&messages, "hello", 11_500));
        assert!(!is_ai_echo(&messages, "hello", 20_000), "too old");
        assert!(!is_ai_echo(&messages, "other", 10_500));
    }
}
