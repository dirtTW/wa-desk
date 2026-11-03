//! Chat-store housekeeping: folding duplicate conversations, trimming the
//! message cache and the profile-photo queue rules.
//!
//! The webview owns the `Map`/arrays; these functions return *plans* that the
//! webview applies, so no message data has to round-trip.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::text::is_placeholder_name;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MergeChatView {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub is_group: bool,
    /// Message ids already in the chat, in order.
    #[serde(default)]
    pub message_ids: Vec<String>,
    #[serde(default)]
    pub unread: i64,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub cleared: bool,
    #[serde(default)]
    pub last_ts: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MergeOp {
    /// The duplicate that disappears.
    pub source: String,
    /// The conversation that survives.
    pub target: String,
    /// Source message ids the target does not have yet, in source order.
    pub append_ids: Vec<String>,
    /// The source's unread count, to add to the target.
    pub unread_add: i64,
    /// Resulting target flags (a surviving copy means not deleted/cleared).
    pub deleted: bool,
    pub cleared: bool,
    /// Resulting `lastTs` when the source is newer, otherwise `None`.
    pub last_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MergePlan {
    pub changed: bool,
    pub ops: Vec<MergeOp>,
    /// Dropped jid -> surviving jid, so the open chat can follow the merge.
    pub remap: Vec<(String, String)>,
}

struct Entry {
    jid: String,
    name: String,
    ids: Vec<String>,
    unread: i64,
    deleted: bool,
    cleared: bool,
    last_ts: i64,
    alive: bool,
}

/// Fold DMs that share a display name (PN vs LID, or a re-sync). Groups,
/// channels and placeholder names are never folded, and the richer copy wins.
pub fn merge_plan(chats: &[MergeChatView]) -> MergePlan {
    let mut entries: Vec<Entry> = chats
        .iter()
        .map(|chat| Entry {
            jid: chat.jid.clone(),
            name: chat.name.clone(),
            ids: chat.message_ids.clone(),
            unread: chat.unread,
            deleted: chat.deleted,
            cleared: chat.cleared,
            last_ts: chat.last_ts,
            alive: true,
        })
        .collect();

    let mut by_name: Vec<(String, usize)> = Vec::new();
    let mut ops: Vec<MergeOp> = Vec::new();
    let mut remap: Vec<(String, String)> = Vec::new();

    // Iteration order mirrors the Map: first occurrence becomes the keeper.
    for index in 0..entries.len() {
        if !entries[index].alive {
            continue;
        }
        let is_group = chats.get(index).map(|chat| chat.is_group).unwrap_or(false);
        if is_group {
            continue;
        }
        let jid = entries[index].jid.clone();
        let name = entries[index].name.clone();
        if jid.ends_with("@newsletter")
            || jid.ends_with("@g.us")
            || is_placeholder_name(&jid, &name)
        {
            continue;
        }
        let key = name.trim().to_lowercase();
        if key.is_empty() {
            continue;
        }

        let Some(existing) = by_name
            .iter()
            .find(|(entry, _)| *entry == key)
            .map(|(_, index)| *index)
        else {
            by_name.push((key, index));
            continue;
        };

        // Same count: the phone-number chat wins over a LID chat. Otherwise the
        // conversation with more messages wins.
        let existing_is_lid = entries[existing].jid.ends_with("@lid");
        let current_is_lid = jid.ends_with("@lid");
        let (target, source) = if entries[existing].ids.len() == entries[index].ids.len() {
            if existing_is_lid && !current_is_lid {
                (index, existing)
            } else {
                (existing, index)
            }
        } else if entries[existing].ids.len() > entries[index].ids.len() {
            (existing, index)
        } else {
            (index, existing)
        };

        let append_ids: Vec<String> = entries[source]
            .ids
            .iter()
            .filter(|id| !entries[target].ids.contains(id))
            .cloned()
            .collect();
        for id in &append_ids {
            entries[target].ids.push(id.clone());
        }
        // Messages are ordered by timestamp in the webview; ids stay unique.
        entries[target].unread += entries[source].unread;
        entries[target].deleted = entries[target].deleted && entries[source].deleted;
        entries[target].cleared = entries[target].cleared && entries[source].cleared;
        let last_ts = if entries[source].last_ts > entries[target].last_ts {
            entries[target].last_ts = entries[source].last_ts;
            Some(entries[source].last_ts)
        } else {
            None
        };

        let surviving = entries[target].jid.clone();
        let dropped = entries[source].jid.clone();
        ops.push(MergeOp {
            source: dropped.clone(),
            target: surviving.clone(),
            append_ids,
            unread_add: entries[source].unread,
            deleted: entries[target].deleted,
            cleared: entries[target].cleared,
            last_ts,
        });
        entries[source].alive = false;
        remap.push((dropped, surviving.clone()));
        if let Some(slot) = by_name.iter_mut().find(|(entry, _)| *entry == key) {
            slot.1 = target;
        } else {
            by_name.push((key, target));
        }
    }

    MergePlan {
        changed: !ops.is_empty(),
        ops,
        remap,
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TrimChatView {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub len: usize,
    #[serde(default)]
    pub last_ts: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TrimOp {
    pub jid: String,
    /// Messages to drop from the front of this chat.
    pub drop: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrimPlan {
    pub ops: Vec<TrimOp>,
    pub dropped: usize,
}

/// Per-chat cap first, then a global budget trimming the oldest chats while
/// never touching `keep_jid`.
pub fn trim_plan(
    chats: &[TrimChatView],
    per_chat: usize,
    budget: usize,
    keep_jid: &str,
) -> TrimPlan {
    let mut sizes: Vec<usize> = chats.iter().map(|chat| chat.len).collect();
    let mut total = 0usize;
    let mut ops: Vec<TrimOp> = Vec::new();

    for (index, size) in sizes.iter_mut().enumerate() {
        if *size > per_chat {
            let drop = *size - per_chat;
            ops.push(TrimOp {
                jid: chats[index].jid.clone(),
                drop,
            });
            *size = per_chat;
        }
        total += *size;
    }

    let mut dropped: usize = ops.iter().map(|op| op.drop).sum();
    if total <= budget {
        return TrimPlan { ops, dropped };
    }

    let mut order: Vec<usize> = (0..chats.len())
        .filter(|index| chats[*index].jid != keep_jid)
        .collect();
    order.sort_by_key(|index| chats[*index].last_ts);

    for index in order {
        if total <= budget {
            break;
        }
        let overflow = sizes[index].min(total - budget);
        if overflow == 0 {
            continue;
        }
        match ops.iter_mut().find(|op| op.jid == chats[index].jid) {
            Some(op) => op.drop += overflow,
            None => ops.push(TrimOp {
                jid: chats[index].jid.clone(),
                drop: overflow,
            }),
        }
        total -= overflow;
        dropped += overflow;
        sizes[index] -= overflow;
    }

    TrimPlan { ops, dropped }
}

/// Whether `queuePhoto` may enqueue this JID.
pub fn photo_queue_allowed(
    jid: &str,
    connected: bool,
    has_photo: bool,
    failed: bool,
    queued: bool,
) -> bool {
    if !connected || jid.is_empty() {
        return false;
    }
    if jid.starts_with("0@") || jid.ends_with("@broadcast") {
        return false;
    }
    !(has_photo || failed || queued)
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SeedContact {
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeededChat {
    pub jid: String,
    pub name: String,
}

/// Rebuild personal chats from the address book when the cache is empty: a
/// phone with at least six digits, not our own number, not already known.
pub fn seed_plan(contacts: &[SeedContact], me_phone: &str, existing: &[String]) -> Vec<SeededChat> {
    let mut out = Vec::new();
    let mut seen: HashSet<String> = existing.iter().cloned().collect();
    for contact in contacts {
        let phone = contact.phone.trim();
        if phone.len() < 6 || phone == me_phone || contact.name.is_empty() {
            continue;
        }
        if !phone.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let jid = format!("{phone}@s.whatsapp.net");
        if !seen.insert(jid.clone()) {
            continue;
        }
        out.push(SeededChat {
            jid,
            name: contact.name.clone(),
        });
    }
    out
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AliasChat {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub exists: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AliasLookup {
    /// The chat the lookup was for.
    #[serde(default)]
    pub chat: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub lid: String,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AliasFold {
    pub from: String,
    pub to: String,
    pub name: String,
}

#[derive(Debug, Default, Serialize)]
pub struct AliasPlan {
    pub folds: Vec<AliasFold>,
    pub renames: Vec<(String, String)>,
    pub changed: bool,
}

/// Fold the LID row into the phone-number row (the canonical DM key) and rename
/// placeholder rows with the verified identity.
pub fn peer_fold_plan(plan: &[AliasLookup], chats: &[AliasChat]) -> AliasPlan {
    let mut out = AliasPlan::default();
    let exists = |jid: &str| chats.iter().any(|chat| chat.jid == jid && chat.exists);
    let name_of = |jid: &str| {
        chats
            .iter()
            .find(|chat| chat.jid == jid)
            .map(|chat| chat.name.clone())
            .unwrap_or_default()
    };
    let mut folded: HashSet<String> = HashSet::new();

    for lookup in plan {
        let phone_jid = if lookup.phone.is_empty() {
            String::new()
        } else {
            format!("{}@s.whatsapp.net", lookup.phone)
        };
        let lid_jid = if lookup.lid.is_empty() {
            String::new()
        } else {
            format!("{}@lid", lookup.lid)
        };
        let target = if phone_jid.is_empty() {
            lookup.chat.clone()
        } else {
            phone_jid.clone()
        };
        if target.is_empty() {
            continue;
        }

        if !phone_jid.is_empty() {
            for key in [lookup.chat.clone(), lid_jid] {
                if key.is_empty() || key == target || !exists(&key) {
                    continue;
                }
                if folded.insert(key.clone()) {
                    out.folds.push(AliasFold {
                        from: key,
                        to: target.clone(),
                        name: lookup.name.clone(),
                    });
                }
            }
        }

        if !lookup.name.is_empty() {
            let current = name_of(&target);
            let placeholder =
                current.is_empty() || crate::text::is_placeholder_name(&target, &current);
            if placeholder && current != lookup.name {
                out.renames.push((target.clone(), lookup.name.clone()));
            }
        }
    }

    out.changed = !out.folds.is_empty() || !out.renames.is_empty();
    out
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContactChatView {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    /// The account's own chat, which is never renamed by a contact.
    #[serde(default)]
    pub is_self: bool,
    /// Distinct sender JIDs already seen in this chat.
    #[serde(default)]
    pub senders: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ContactPlan {
    /// Chats whose display name becomes the saved contact name.
    pub chat_renames: Vec<(String, String)>,
    /// (chat jid, sender jid, name) for every sender to relabel.
    pub sender_renames: Vec<(String, String, String)>,
    pub changed: bool,
}

/// Apply the address book to the chat store: rename a one-to-one chat whose user
/// part matches, and every sender bubble in any chat.
pub fn contact_rename_plan(updates: &[(String, String)], chats: &[ContactChatView]) -> ContactPlan {
    let mut plan = ContactPlan::default();
    for (key, name) in updates {
        if key.is_empty() || name.is_empty() {
            continue;
        }
        for chat in chats {
            if !chat.is_self && crate::text::phone_from_jid(&chat.jid) == *key {
                plan.chat_renames.push((chat.jid.clone(), name.clone()));
            }
            for sender in &chat.senders {
                if crate::text::phone_from_jid(sender) == *key {
                    plan.sender_renames
                        .push((chat.jid.clone(), sender.clone(), name.clone()));
                }
            }
        }
    }
    plan.changed = !plan.chat_renames.is_empty() || !plan.sender_renames.is_empty();
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(jid: &str, name: &str, ids: &[&str], unread: i64) -> MergeChatView {
        MergeChatView {
            jid: jid.into(),
            name: name.into(),
            message_ids: ids.iter().map(|id| id.to_string()).collect(),
            unread,
            last_ts: ids.len() as i64 * 1000,
            ..Default::default()
        }
    }

    #[test]
    fn merges_lid_and_pn_by_name() {
        let plan = merge_plan(&[
            chat("111@lid", "Alice", &["a"], 2),
            chat("6285@s.whatsapp.net", "Alice", &["b"], 0),
            chat("1@g.us", "Alice", &["g"], 0),
        ]);
        assert!(plan.changed);
        assert_eq!(plan.ops.len(), 1);
        let op = &plan.ops[0];
        assert_eq!(op.target, "6285@s.whatsapp.net", "PN copy wins");
        assert_eq!(op.source, "111@lid");
        assert_eq!(op.append_ids, vec!["a".to_string()]);
        assert_eq!(op.unread_add, 2);
        assert_eq!(
            plan.remap,
            vec![("111@lid".to_string(), "6285@s.whatsapp.net".to_string())]
        );
    }

    #[test]
    fn richer_chat_wins_and_ids_are_deduped() {
        let plan = merge_plan(&[
            chat("a@s.whatsapp.net", "Bob", &["1"], 0),
            chat("a@lid", "Bob", &["1", "2", "3"], 5),
        ]);
        assert_eq!(plan.ops[0].target, "a@lid");
        assert_eq!(plan.ops[0].append_ids, Vec::<String>::new());
        assert_eq!(plan.ops[0].unread_add, 0);
    }

    #[test]
    fn placeholder_channels_and_groups_are_not_merged() {
        assert!(!merge_plan(&[chat("1@s.whatsapp.net", "1", &["a"], 0)]).changed);
        assert!(!merge_plan(&[chat("1@newsletter", "News", &["a"], 0)]).changed);
        let mut group = chat("2@g.us", "Team", &["a"], 0);
        group.is_group = true;
        assert!(!merge_plan(&[group]).changed);
        assert!(!merge_plan(&[chat("a@s.whatsapp.net", "", &["a"], 0)]).changed);
    }

    #[test]
    fn deletion_survives_only_when_both_copies_are_deleted() {
        let mut a = chat("a@s.whatsapp.net", "Cara", &["1"], 0);
        a.deleted = true;
        let mut b = chat("b@lid", "Cara", &["2"], 0);
        b.deleted = false;
        let plan = merge_plan(&[a, b]);
        assert!(!plan.ops[0].deleted);
    }

    #[test]
    fn seeding_skips_self_short_and_known_numbers() {
        let contacts = vec![
            SeedContact {
                phone: "628770000000".into(),
                name: "Budi".into(),
            },
            SeedContact {
                phone: "62111".into(),
                name: "Short".into(),
            },
            SeedContact {
                phone: "628550000000".into(),
                name: "Me".into(),
            },
            SeedContact {
                phone: "628990000000".into(),
                name: "Known".into(),
            },
            SeedContact {
                phone: "not-a-number".into(),
                name: "Bad".into(),
            },
        ];
        let existing = vec!["628990000000@s.whatsapp.net".to_string()];
        let out = seed_plan(&contacts, "628550000000", &existing);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].jid, "628770000000@s.whatsapp.net");
        assert_eq!(out[0].name, "Budi");
    }

    #[test]
    fn peer_plan_folds_lid_into_phone_and_renames_placeholders() {
        let chats = vec![
            AliasChat {
                jid: "111@lid".into(),
                name: "111".into(),
                exists: true,
            },
            AliasChat {
                jid: "6285@s.whatsapp.net".into(),
                name: "6285".into(),
                exists: true,
            },
        ];
        let plan = peer_fold_plan(
            &[AliasLookup {
                chat: "111@lid".into(),
                phone: "6285".into(),
                lid: "111".into(),
                name: "Ada".into(),
            }],
            &chats,
        );
        assert!(plan.changed);
        assert_eq!(
            plan.folds,
            vec![AliasFold {
                from: "111@lid".into(),
                to: "6285@s.whatsapp.net".into(),
                name: "Ada".into(),
            }]
        );
        assert_eq!(
            plan.renames,
            vec![("6285@s.whatsapp.net".to_string(), "Ada".to_string())]
        );
    }

    #[test]
    fn peer_plan_keeps_real_names() {
        let chats = vec![AliasChat {
            jid: "6285@s.whatsapp.net".into(),
            name: "Ada".into(),
            exists: true,
        }];
        let plan = peer_fold_plan(
            &[AliasLookup {
                chat: "6285@s.whatsapp.net".into(),
                phone: "6285".into(),
                lid: String::new(),
                name: "Ada".into(),
            }],
            &chats,
        );
        assert!(!plan.changed, "nothing to fold or rename");
        assert!(plan.folds.is_empty());
    }

    #[test]
    fn peer_plan_without_a_phone_uses_the_chat_itself() {
        let chats = vec![AliasChat {
            jid: "6285@s.whatsapp.net".into(),
            name: "6285".into(),
            exists: true,
        }];
        let plan = peer_fold_plan(
            &[AliasLookup {
                chat: "6285@s.whatsapp.net".into(),
                phone: String::new(),
                lid: String::new(),
                name: "Ada".into(),
            }],
            &chats,
        );
        assert!(plan.folds.is_empty());
        assert_eq!(
            plan.renames,
            vec![("6285@s.whatsapp.net".to_string(), "Ada".to_string())]
        );
    }

    #[test]
    fn contact_plan_renames_chats_and_senders() {
        let chats = vec![
            ContactChatView {
                jid: "628770000000@s.whatsapp.net".into(),
                name: "628770000000".into(),
                is_self: false,
                senders: Vec::new(),
            },
            ContactChatView {
                jid: "1@g.us".into(),
                name: "Team".into(),
                is_self: false,
                senders: vec![
                    "628770000000@s.whatsapp.net".into(),
                    "628990000000@s.whatsapp.net".into(),
                ],
            },
            ContactChatView {
                jid: "628770000000@s.whatsapp.net".into(),
                name: "Me".into(),
                is_self: true,
                senders: Vec::new(),
            },
        ];
        let plan = contact_rename_plan(&[("628770000000".to_string(), "Budi".to_string())], &chats);
        assert!(plan.changed);
        assert_eq!(
            plan.chat_renames,
            vec![(
                "628770000000@s.whatsapp.net".to_string(),
                "Budi".to_string()
            )]
        );
        assert_eq!(
            plan.sender_renames,
            vec![(
                "1@g.us".to_string(),
                "628770000000@s.whatsapp.net".to_string(),
                "Budi".to_string()
            )]
        );

        let empty = contact_rename_plan(&[("1".into(), String::new())], &chats);
        assert!(!empty.changed);
    }

    #[test]
    fn trimming_caps_then_drops_the_oldest() {
        let chats = vec![
            TrimChatView {
                jid: "a".into(),
                len: 10,
                last_ts: 100,
            },
            TrimChatView {
                jid: "b".into(),
                len: 6,
                last_ts: 200,
            },
            TrimChatView {
                jid: "keep".into(),
                len: 4,
                last_ts: 50,
            },
        ];
        let plan = trim_plan(&chats, 8, 12, "keep");
        // a is capped 10 -> 8 (2 dropped); total 18 > 12, so trim the oldest
        // non-kept chat (a) by 6 more, then b is untouched.
        assert_eq!(plan.dropped, 8);
        assert!(plan.ops.contains(&TrimOp {
            jid: "a".into(),
            drop: 8
        }));
        assert!(!plan.ops.iter().any(|op| op.jid == "keep"));
    }

    #[test]
    fn trimming_keeps_the_open_chat_and_budget() {
        let chats = vec![
            TrimChatView {
                jid: "a".into(),
                len: 5,
                last_ts: 1,
            },
            TrimChatView {
                jid: "b".into(),
                len: 5,
                last_ts: 2,
            },
        ];
        let plan = trim_plan(&chats, 10, 6, "a");
        assert_eq!(plan.dropped, 4);
        assert_eq!(
            plan.ops,
            vec![TrimOp {
                jid: "b".into(),
                drop: 4
            }]
        );

        let none = trim_plan(&chats, 10, 100, "a");
        assert_eq!(none.dropped, 0);
        assert!(none.ops.is_empty());
    }

    #[test]
    fn photo_queue_rules() {
        assert!(photo_queue_allowed(
            "6285@s.whatsapp.net",
            true,
            false,
            false,
            false
        ));
        assert!(!photo_queue_allowed(
            "6285@s.whatsapp.net",
            false,
            false,
            false,
            false
        ));
        assert!(!photo_queue_allowed(
            "6285@s.whatsapp.net",
            true,
            true,
            false,
            false
        ));
        assert!(!photo_queue_allowed(
            "6285@s.whatsapp.net",
            true,
            false,
            true,
            false
        ));
        assert!(!photo_queue_allowed(
            "6285@s.whatsapp.net",
            true,
            false,
            false,
            true
        ));
        assert!(!photo_queue_allowed(
            "0@s.whatsapp.net",
            true,
            false,
            false,
            false
        ));
        assert!(!photo_queue_allowed(
            "status@broadcast",
            true,
            false,
            false,
            false
        ));
        assert!(!photo_queue_allowed("", true, false, false, false));
    }
}
