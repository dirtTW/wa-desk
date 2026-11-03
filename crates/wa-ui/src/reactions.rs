//! Emoji reactions: the toggle decision, the fold of a live reaction event and
//! the rows after our own send.

use serde::{Deserialize, Serialize};

/// One reaction pill. `by` is `"me"` for our own reaction, otherwise the
/// author's phone number.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct ReactionRow {
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub ts: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReactionToggle {
    /// The emoji to send; empty removes our reaction.
    pub value: String,
    /// A changed reaction revokes the old one first: the server does not apply a
    /// plain replacement consistently.
    pub revoke_first: bool,
    /// What our pill shows after the tap (empty = no pill of ours).
    pub mine: String,
}

/// Tapping an emoji: the same one removes our reaction, a different one replaces
/// it, and nothing yet means a plain add.
pub fn toggle(my: Option<&ReactionRow>, tapped: &str) -> ReactionToggle {
    let tapped = tapped.trim();
    let current = my.map(|row| row.emoji.as_str()).unwrap_or("");
    if tapped.is_empty() || current == tapped {
        return ReactionToggle {
            value: String::new(),
            revoke_first: false,
            mine: String::new(),
        };
    }
    ReactionToggle {
        value: tapped.to_string(),
        revoke_first: !current.is_empty(),
        mine: tapped.to_string(),
    }
}

/// The rows after our send resolves: ours is replaced (or dropped when `value`
/// is empty), everyone else's is untouched.
pub fn after_send(rows: &[ReactionRow], value: &str, now: i64) -> Vec<ReactionRow> {
    let mut next: Vec<ReactionRow> = rows.iter().filter(|row| row.by != "me").cloned().collect();
    if !value.trim().is_empty() {
        next.push(ReactionRow {
            by: "me".to_string(),
            emoji: value.trim().to_string(),
            ts: now,
        });
    }
    next
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReactionEventPlan {
    /// A delayed echo of an older reaction: the pill must not flip back.
    pub apply: bool,
    pub rows: Vec<ReactionRow>,
}

/// Fold a live `reaction` event for one author. An older `ts` than the row we
/// already hold is a delayed echo and is ignored; an empty emoji removes it.
pub fn apply_event(rows: &[ReactionRow], by: &str, emoji: &str, ts: i64) -> ReactionEventPlan {
    let by = by.trim();
    if by.is_empty() {
        return ReactionEventPlan {
            apply: false,
            rows: rows.to_vec(),
        };
    }
    let stale = rows
        .iter()
        .find(|row| row.by == by)
        .is_some_and(|existing| ts > 0 && existing.ts > 0 && ts < existing.ts);
    if stale {
        return ReactionEventPlan {
            apply: false,
            rows: rows.to_vec(),
        };
    }
    let mut next: Vec<ReactionRow> = rows.iter().filter(|row| row.by != by).cloned().collect();
    let emoji = emoji.trim();
    if !emoji.is_empty() {
        next.push(ReactionRow {
            by: by.to_string(),
            emoji: emoji.to_string(),
            ts,
        });
    }
    ReactionEventPlan {
        apply: true,
        rows: next,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mine(emoji: &str) -> ReactionRow {
        ReactionRow {
            by: "me".into(),
            emoji: emoji.into(),
            ts: 10,
        }
    }

    #[test]
    fn tapping_toggles_adds_and_changes() {
        // Nothing yet: a plain add.
        let add = toggle(None, "👍");
        assert_eq!(add.value, "👍");
        assert!(!add.revoke_first);
        assert_eq!(add.mine, "👍");

        // Tapping the same emoji removes it.
        let remove = toggle(Some(&mine("👍")), "👍");
        assert!(remove.value.is_empty() && remove.mine.is_empty());
        assert!(!remove.revoke_first, "removing needs no revoke");

        // A different emoji replaces it, revoking first.
        let change = toggle(Some(&mine("👍")), "❤️");
        assert_eq!(change.value, "❤️");
        assert!(change.revoke_first);
        assert_eq!(change.mine, "❤️");

        // Surrounding whitespace from the picker is ignored.
        assert_eq!(toggle(None, "  😂  ").value, "😂");
        // An empty tap is a removal.
        assert!(toggle(Some(&mine("👍")), "").value.is_empty());
    }

    #[test]
    fn our_send_replaces_only_our_row() {
        let rows = vec![
            ReactionRow {
                by: "6285@s.whatsapp.net".into(),
                emoji: "👍".into(),
                ts: 5,
            },
            mine("❤️"),
        ];
        let after = after_send(&rows, "😂", 99);
        assert_eq!(after.len(), 2);
        assert_eq!(
            after[0].by, "6285@s.whatsapp.net",
            "others keep their order"
        );
        assert_eq!(after[1].emoji, "😂");
        assert_eq!(after[1].ts, 99);

        let removed = after_send(&rows, "", 99);
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].by, "6285@s.whatsapp.net");
    }

    #[test]
    fn live_events_replace_the_author_and_ignore_delayed_echoes() {
        let rows = vec![
            ReactionRow {
                by: "111@s.whatsapp.net".into(),
                emoji: "👍".into(),
                ts: 50,
            },
            ReactionRow {
                by: "222@s.whatsapp.net".into(),
                emoji: "❤️".into(),
                ts: 60,
            },
        ];

        // A newer reaction from the first author replaces their row (moved to
        // the end, since it is re-added).
        let plan = apply_event(&rows, "111@s.whatsapp.net", "🔥", 70);
        assert!(plan.apply);
        assert_eq!(plan.rows.len(), 2);
        assert_eq!(
            plan.rows[0].by, "222@s.whatsapp.net",
            "the other author keeps its place"
        );
        assert_eq!(plan.rows[0].emoji, "❤️");
        assert_eq!(plan.rows[1].emoji, "🔥");

        // An older echo is ignored.
        let stale = apply_event(&rows, "111@s.whatsapp.net", "😂", 40);
        assert!(!stale.apply);
        assert_eq!(stale.rows, rows);

        // An empty emoji removes that author's row.
        let removed = apply_event(&rows, "222@s.whatsapp.net", "", 80);
        assert!(removed.apply);
        assert_eq!(removed.rows.len(), 1);
        assert_eq!(removed.rows[0].by, "111@s.whatsapp.net");

        // No author at all: nothing happens.
        assert!(!apply_event(&rows, "", "👍", 90).apply);
    }
}
