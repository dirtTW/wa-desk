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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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

/// A chat as the webview holds it, before the contact plan needs its view.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContactChatInput {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub senders: Vec<String>,
}

/// Reduce the chat store to what `contact_plan` needs, marking the account's own
/// chat with the same JID rule the rest of the UI uses (`is_self_jid` compares
/// user parts, so a device suffix does not hide it).
pub fn contact_chat_views(
    chats: &[ContactChatInput],
    me_jid: &str,
    me_lid: &str,
) -> Vec<ContactChatView> {
    chats
        .iter()
        .map(|chat| {
            let mut senders = chat.senders.clone();
            senders.retain(|sender| !sender.is_empty());
            senders.sort();
            senders.dedup();
            ContactChatView {
                jid: chat.jid.clone(),
                name: chat.name.clone(),
                is_self: crate::text::is_self_jid(&chat.jid, me_jid, me_lid),
                senders,
            }
        })
        .collect()
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

/// The chat row for a group we just created: no messages yet, newest activity
/// now, nothing unread.
pub fn created_group_chat(jid: &str, name: &str, now: i64) -> serde_json::Value {
    serde_json::json!({
        "jid": jid,
        "name": name,
        "isGroup": true,
        "messages": [],
        "last": serde_json::Value::Null,
        "lastTs": now,
        "unread": 0,
    })
}

/// The address book is requested once per connection and only while we have no
/// saved names yet.
pub fn contact_sync_needed(syncing: bool, known: usize) -> bool {
    !syncing && known == 0
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PeerChatInput {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub is_group: bool,
}

/// Which chats still need a `usync` peer lookup, in store order: private chats
/// we already tried are skipped, LID chats always go, and a phone-number chat
/// only while its name is still the bare number. One call replaces a round trip
/// per chat.
pub fn peer_targets(
    chats: &[PeerChatInput],
    me_jid: &str,
    me_lid: &str,
    seen: &[String],
    limit: usize,
) -> Vec<String> {
    chats
        .iter()
        .filter(|chat| !chat.jid.is_empty() && !chat.is_group)
        .filter(|chat| !crate::text::is_self_jid(&chat.jid, me_jid, me_lid))
        .filter(|chat| !seen.iter().any(|jid| jid == &chat.jid))
        .filter(|chat| {
            chat.jid.ends_with("@lid") || crate::text::is_placeholder_name(&chat.jid, &chat.name)
        })
        .take(limit)
        .map(|chat| chat.jid.clone())
        .collect()
}

/// Profile photos are fetched a few at a time, like WhatsApp Web.
pub const PHOTO_CONCURRENCY: usize = 3;
/// A failed profile photo is retried after this long instead of being hidden
/// for the rest of the session.
pub const PHOTO_RETRY_MS: i64 = 120_000;

#[derive(Debug, Clone, Default, Serialize)]
pub struct PhotoPump {
    /// The JIDs whose fetch should start now.
    pub start: Vec<String>,
    /// How many remain in flight after starting them.
    pub in_flight: usize,
    /// How many are still waiting behind them.
    pub waiting: usize,
}

/// Fill the photo fetcher up to its concurrency limit.
pub fn photo_pump(
    queue: &[String],
    in_flight: usize,
    connected: bool,
    concurrency: usize,
) -> PhotoPump {
    if !connected || concurrency == 0 {
        return PhotoPump {
            start: Vec::new(),
            in_flight,
            waiting: queue.len(),
        };
    }
    let free = concurrency.saturating_sub(in_flight);
    let take = free.min(queue.len());
    PhotoPump {
        start: queue.iter().take(take).cloned().collect(),
        in_flight: in_flight + take,
        waiting: queue.len() - take,
    }
}

/// Only a real CDN URL may be cached; anything else (empty, a stale `data:` or
/// `blob:` blob) would be persisted and then fail to paint on the next start.
pub fn photo_result_ok(url: &str) -> bool {
    url.starts_with("http")
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UpsertInput {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub is_group: Option<bool>,
    /// The chat is already in the store.
    #[serde(default)]
    pub exists: bool,
    /// Its current display name (only read when `exists`).
    #[serde(default)]
    pub current_name: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct UpsertPlan {
    /// Create the row (with `name` as the initial display name).
    pub create: bool,
    pub name: String,
    pub is_group: bool,
    /// The existing row's name changes to `name`.
    pub rename: bool,
    /// The existing row's group flag changes.
    pub set_group: bool,
}

/// Decide what a message/contact event does to the chat row: create it with the
/// JID user part as the fallback name, rename a placeholder once a real name
/// arrives, and keep the group flag in sync.
pub fn upsert_plan(input: &UpsertInput) -> UpsertPlan {
    let fallback = input.jid.split('@').next().unwrap_or("").to_string();
    let wanted = if input.name.is_empty() {
        fallback.clone()
    } else {
        input.name.clone()
    };
    if !input.exists {
        return UpsertPlan {
            create: true,
            name: wanted,
            is_group: input.is_group.unwrap_or(false),
            rename: false,
            set_group: false,
        };
    }
    let rename = !input.name.is_empty()
        && input.current_name != input.name
        && crate::text::is_placeholder_name(&input.jid, &input.current_name);
    UpsertPlan {
        create: false,
        name: wanted,
        is_group: input.is_group.unwrap_or(false),
        rename,
        set_group: input.is_group.is_some(),
    }
}

/// Where a message belongs in a timestamp-sorted list (stable: equal timestamps
/// keep their arrival order).
pub fn insert_index(timestamps: &[i64], timestamp: i64) -> usize {
    match timestamps.iter().position(|stamp| *stamp > timestamp) {
        Some(index) => index,
        None => timestamps.len(),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FoldInput {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
    /// The verified identity name, when the lookup returned one.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub source_exists: bool,
    #[serde(default)]
    pub target_exists: bool,
    #[serde(default)]
    pub target_name: String,
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub target_ids: Vec<String>,
    #[serde(default)]
    pub source_last_ts: i64,
    #[serde(default)]
    pub target_last_ts: i64,
    #[serde(default)]
    pub source_unread: i64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FoldMode {
    /// Nothing to do.
    None,
    /// Only the surviving row takes the verified name.
    RenameOnly,
    /// The source key moves to `to` (the target row never existed).
    Rekey,
    /// The source's messages move into the target row.
    Merge,
}

#[derive(Debug, Clone, Serialize)]
pub struct FoldPlan {
    pub mode: FoldMode,
    /// Source message ids the target does not have yet, in source order.
    pub append_ids: Vec<String>,
    /// The merged row's `lastTs`, when the source was newer.
    pub last_ts: Option<i64>,
    pub unread_add: i64,
    /// The verified name to apply (empty when unchanged).
    pub name: String,
}

/// Fold one chat key into another after a peer lookup: rename in place, re-key
/// an empty target, or merge the message sets. A name only replaces a row whose
/// display name is still the bare JID user part.
pub fn fold_plan(input: &FoldInput) -> FoldPlan {
    let none = FoldPlan {
        mode: FoldMode::None,
        append_ids: Vec::new(),
        last_ts: None,
        unread_add: 0,
        name: String::new(),
    };
    let wants_rename = !input.name.is_empty() && input.target_name != input.name;

    if input.from == input.to {
        if !input.target_exists || !wants_rename {
            return none;
        }
        return FoldPlan {
            mode: FoldMode::RenameOnly,
            name: input.name.clone(),
            ..none
        };
    }
    if input.from.is_empty() || input.to.is_empty() {
        return none;
    }

    if !input.source_exists {
        // Only a placeholder row may take the verified name.
        if input.target_exists
            && !input.name.is_empty()
            && crate::text::is_placeholder_name(&input.to, &input.target_name)
        {
            return FoldPlan {
                mode: FoldMode::RenameOnly,
                name: input.name.clone(),
                ..none
            };
        }
        return none;
    }

    if !input.target_exists {
        return FoldPlan {
            mode: FoldMode::Rekey,
            name: if input.name.is_empty() {
                String::new()
            } else {
                input.name.clone()
            },
            ..none
        };
    }

    let append_ids: Vec<String> = input
        .source_ids
        .iter()
        .filter(|id| !input.target_ids.contains(id))
        .cloned()
        .collect();
    let last_ts = if input.source_last_ts > input.target_last_ts {
        Some(input.source_last_ts)
    } else {
        None
    };
    FoldPlan {
        mode: FoldMode::Merge,
        append_ids,
        last_ts,
        unread_add: input.source_unread,
        name: input.name.clone(),
    }
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
    fn contact_views_mark_the_own_chat_and_dedupe_senders() {
        let chats = vec![
            ContactChatInput {
                jid: "62111@s.whatsapp.net".into(),
                name: "Me".into(),
                senders: vec![
                    "a@s.whatsapp.net".into(),
                    "a@s.whatsapp.net".into(),
                    String::new(),
                ],
            },
            ContactChatInput {
                jid: "62111:7@lid".into(),
                name: "Me (LID)".into(),
                senders: Vec::new(),
            },
            ContactChatInput {
                jid: "6285@s.whatsapp.net".into(),
                name: "Budi".into(),
                senders: vec!["6285@s.whatsapp.net".into()],
            },
        ];
        let views = contact_chat_views(&chats, "62111@s.whatsapp.net", "62111:7@lid");
        assert_eq!(views.len(), 3);
        // The same number with a device suffix is still our own chat.
        assert!(views[0].is_self);
        assert!(views[1].is_self, "the LID form resolves to the same user");
        assert!(!views[2].is_self);
        assert_eq!(
            views[0].senders,
            vec!["a@s.whatsapp.net".to_string()],
            "deduped"
        );
        assert!(views[1].senders.is_empty());
    }

    #[test]
    fn peer_lookups_target_lids_and_placeholder_phones() {
        let chat = |jid: &str, name: &str, is_group: bool| PeerChatInput {
            jid: jid.into(),
            name: name.into(),
            is_group,
        };
        let chats = vec![
            chat("111@lid", "111", false),
            chat("222@s.whatsapp.net", "222", false),
            chat("333@s.whatsapp.net", "Budi", false),
            chat("444@g.us", "Team", true),
            chat("555@s.whatsapp.net", "555", false),
            chat("666@lid", "666", false),
        ];
        let targets = peer_targets(
            &chats,
            "999@s.whatsapp.net",
            "999@lid",
            &["666@lid".to_string()],
            100,
        );
        assert_eq!(
            targets,
            vec![
                "111@lid".to_string(),
                "222@s.whatsapp.net".to_string(),
                "555@s.whatsapp.net".to_string()
            ],
            "groups, named chats and already-tried jids are skipped"
        );

        // Our own ids never go; another person's LID is still a target.
        let mine = vec![
            chat("999@s.whatsapp.net", "999", false),
            chat("999@lid", "999", false),
            chat("111@lid", "111", false),
        ];
        assert_eq!(
            peer_targets(&mine, "999@s.whatsapp.net", "999@lid", &[], 100),
            vec!["111@lid".to_string()]
        );
        let many: Vec<PeerChatInput> = (0..10)
            .map(|i| chat(&format!("{i}@lid"), "x", false))
            .collect();
        assert_eq!(peer_targets(&many, "", "", &[], 4).len(), 4);
    }

    #[test]
    fn a_created_group_starts_empty_and_unread_free() {
        let row = created_group_chat("123@g.us", "Team", 42);
        assert_eq!(row["jid"], "123@g.us");
        assert_eq!(row["name"], "Team");
        assert_eq!(row["isGroup"], true);
        assert_eq!(row["unread"], 0);
        assert_eq!(row["lastTs"], 42);
        assert_eq!(row["messages"].as_array().map(Vec::len), Some(0));
        assert!(row["last"].is_null());
    }

    #[test]
    fn the_address_book_is_requested_once_while_empty() {
        assert!(contact_sync_needed(false, 0), "nothing stored yet");
        assert!(!contact_sync_needed(true, 0), "already in flight");
        assert!(!contact_sync_needed(false, 12), "names already cached");
    }

    #[test]
    fn photo_pump_fills_the_concurrency_window() {
        let queue: Vec<String> = (0..5).map(|i| format!("p{i}")).collect();
        let pump = photo_pump(&queue, 0, true, PHOTO_CONCURRENCY);
        assert_eq!(pump.start, vec!["p0", "p1", "p2"]);
        assert_eq!(pump.in_flight, 3);
        assert_eq!(pump.waiting, 2);

        // Two already running: only one slot is free.
        let pump = photo_pump(&queue, 2, true, PHOTO_CONCURRENCY);
        assert_eq!(pump.start, vec!["p0"]);
        assert_eq!(pump.in_flight, 3);

        // Full or offline: nothing starts.
        assert!(
            photo_pump(&queue, 3, true, PHOTO_CONCURRENCY)
                .start
                .is_empty()
        );
        let offline = photo_pump(&queue, 0, false, PHOTO_CONCURRENCY);
        assert!(offline.start.is_empty());
        assert_eq!(offline.waiting, 5);
        assert!(photo_pump(&[], 0, true, PHOTO_CONCURRENCY).start.is_empty());
    }

    #[test]
    fn only_http_photo_urls_are_cached() {
        assert!(photo_result_ok("https://pps.whatsapp.net/x.jpg"));
        assert!(photo_result_ok("http://pps/x.jpg"));
        assert!(!photo_result_ok(""));
        assert!(!photo_result_ok("blob:abc"));
        assert!(!photo_result_ok("data:image/jpeg;base64,AA"));
        assert_eq!(PHOTO_RETRY_MS, 120_000);
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
    fn fold_plans_rename_rekey_and_merge() {
        // Both rows exist: merge the missing ids, sum unread, keep the newer ts.
        let merge = fold_plan(&FoldInput {
            from: "111@lid".into(),
            to: "6285@s.whatsapp.net".into(),
            name: "Ada".into(),
            source_exists: true,
            target_exists: true,
            target_name: "6285".into(),
            source_ids: vec!["a".into(), "b".into()],
            target_ids: vec!["b".into(), "c".into()],
            source_last_ts: 500,
            target_last_ts: 400,
            source_unread: 2,
        });
        assert_eq!(merge.mode, FoldMode::Merge);
        assert_eq!(merge.append_ids, vec!["a".to_string()]);
        assert_eq!(merge.last_ts, Some(500));
        assert_eq!(merge.unread_add, 2);
        assert_eq!(merge.name, "Ada");

        // The target never existed: the source takes the new key.
        let rekey = fold_plan(&FoldInput {
            from: "111@lid".into(),
            to: "6285@s.whatsapp.net".into(),
            name: "Ada".into(),
            source_exists: true,
            target_exists: false,
            source_unread: 1,
            ..Default::default()
        });
        assert_eq!(rekey.mode, FoldMode::Rekey);
        assert_eq!(rekey.name, "Ada");

        // Only the target exists and its name is a placeholder: rename.
        let rename = fold_plan(&FoldInput {
            from: "111@lid".into(),
            to: "6285@s.whatsapp.net".into(),
            name: "Ada".into(),
            source_exists: false,
            target_exists: true,
            target_name: "6285".into(),
            ..Default::default()
        });
        assert_eq!(rename.mode, FoldMode::RenameOnly);
        assert_eq!(rename.name, "Ada");

        // A real target name is never overwritten by a lookup.
        let kept = fold_plan(&FoldInput {
            from: "111@lid".into(),
            to: "6285@s.whatsapp.net".into(),
            name: "Ada".into(),
            source_exists: false,
            target_exists: true,
            target_name: "Budi".into(),
            ..Default::default()
        });
        assert_eq!(kept.mode, FoldMode::None);

        // Same key: a rename only.
        let same = fold_plan(&FoldInput {
            from: "6285@s.whatsapp.net".into(),
            to: "6285@s.whatsapp.net".into(),
            name: "Ada".into(),
            source_exists: true,
            target_exists: true,
            target_name: "6285".into(),
            ..Default::default()
        });
        assert_eq!(same.mode, FoldMode::RenameOnly);

        // Nothing anywhere.
        assert_eq!(fold_plan(&FoldInput::default()).mode, FoldMode::None);
    }

    #[test]
    fn upsert_plans_create_rename_and_group_flag() {
        // A brand-new chat: the JID user part is the fallback name.
        let created = upsert_plan(&UpsertInput {
            jid: "6285@s.whatsapp.net".into(),
            name: String::new(),
            is_group: None,
            exists: false,
            current_name: String::new(),
        });
        assert!(created.create);
        assert_eq!(created.name, "6285");
        assert!(!created.is_group);

        // A named group.
        let group = upsert_plan(&UpsertInput {
            jid: "1@g.us".into(),
            name: "Team".into(),
            is_group: Some(true),
            exists: false,
            current_name: String::new(),
        });
        assert_eq!(group.name, "Team");
        assert!(group.is_group);

        // An existing placeholder row takes the real name.
        let renamed = upsert_plan(&UpsertInput {
            jid: "6285@s.whatsapp.net".into(),
            name: "Budi".into(),
            is_group: Some(false),
            exists: true,
            current_name: "6285".into(),
        });
        assert!(!renamed.create && renamed.rename && renamed.set_group);

        // A row that already has a real name keeps it.
        let kept = upsert_plan(&UpsertInput {
            jid: "6285@s.whatsapp.net".into(),
            name: "Someone".into(),
            is_group: Some(false),
            exists: true,
            current_name: "Budi".into(),
        });
        assert!(!kept.rename);

        // No name in the event: nothing to rename.
        let unnamed = upsert_plan(&UpsertInput {
            jid: "6285@s.whatsapp.net".into(),
            name: String::new(),
            is_group: None,
            exists: true,
            current_name: "6285".into(),
        });
        assert!(!unnamed.rename && !unnamed.set_group);
    }

    #[test]
    fn insert_index_keeps_the_list_sorted_and_stable() {
        assert_eq!(insert_index(&[], 100), 0);
        assert_eq!(insert_index(&[100, 200, 300], 50), 0);
        assert_eq!(
            insert_index(&[100, 200, 300], 200),
            2,
            "equal lands after (arrival order)"
        );
        assert_eq!(insert_index(&[100, 200, 300], 250), 2);
        assert_eq!(insert_index(&[100, 200, 300], 400), 3);
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
