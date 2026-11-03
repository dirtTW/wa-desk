//! Secondary sidebar screens rendered from the same chat state: the community
//! screen, the channels panel and the starred-messages list.

use serde::{Deserialize, Serialize};

use crate::html::{avatar, escape, icon};
use crate::labels::stamp_of;
use crate::sidebar::{GroupMeta, SidebarChat, SidebarRequest};
use crate::text::{MessageRow, preview_of};

fn is_self_jid(jid: &str, req: &SidebarRequest) -> bool {
    crate::text::is_self_jid(jid, &req.me_jid, &req.me_lid)
}

fn display_name(name: &str, jid: &str, req: &SidebarRequest) -> String {
    let name = if name.is_empty() {
        jid.split('@').next().unwrap_or("").to_string()
    } else {
        name.to_string()
    };
    if is_self_jid(jid, req) && !name.to_lowercase().ends_with("(you)") {
        return format!("{name} (You)");
    }
    name
}

fn last_ts(chat: &SidebarChat) -> i64 {
    chat.last
        .as_ref()
        .map(|m| m.timestamp)
        .unwrap_or(chat.last_ts)
}

fn preview(message: &MessageRow, chat: &SidebarChat, req: &SidebarRequest) -> String {
    let contact_name = |jid: &str| -> String {
        let phone = crate::text::phone_from_jid(jid);
        req.contacts
            .get(&phone)
            .cloned()
            .or_else(|| {
                req.phones
                    .get(jid)
                    .and_then(|pn| req.contacts.get(pn))
                    .cloned()
            })
            .unwrap_or_default()
    };
    preview_of(message, chat.is_group, req.show_deleted, &contact_name)
}

fn media_icon(kind: &str) -> String {
    let name = match kind {
        "image" => "image",
        "video" => "videocam",
        "audio" => "mic",
        "sticker" => "sticker",
        _ => "description",
    };
    icon(name)
}

/* ---------------------------------------------------------------- channels */

#[derive(Debug, Serialize)]
pub struct ChannelsOutput {
    pub html: String,
    pub empty_hidden: bool,
    pub queue: Vec<String>,
}

/// The channels rail panel: newsletters only, newest first, filtered by the
/// panel's search box.
pub fn channels(req: &SidebarRequest, needle: &str) -> ChannelsOutput {
    let needle = needle.trim().to_lowercase();
    let mut chats: Vec<&SidebarChat> = req
        .chats
        .iter()
        .filter(|chat| chat.jid.ends_with("@newsletter"))
        .filter(|chat| req.show_deleted || !chat.deleted)
        .filter(|chat| needle.is_empty() || chat.name.to_lowercase().contains(&needle))
        .collect();
    chats.sort_by_key(|chat| std::cmp::Reverse(last_ts(chat)));

    let mut html = String::new();
    for chat in &chats {
        let last = chat.last.as_ref();
        let stamp = last.map(|m| m.timestamp).unwrap_or(chat.last_ts);
        let kind = last
            .filter(|m| !m.kind.is_empty() && m.kind != "text")
            .map(|m| m.kind.clone());
        let kind_icon = kind
            .map(|kind| format!(r#"<span class="channel-kind">{}</span>"#, media_icon(&kind)))
            .unwrap_or_default();
        let preview_line = match last {
            Some(message) => format!("{kind_icon}{}", escape(&preview(message, chat, req))),
            None => "Channel".to_string(),
        };
        html.push_str(&format!(
            r#"
        <button type="button" class="channel-row {}" data-jid="{}">
          {}
          <span class="channel-meta">
            <span class="channel-top">
              <strong>{}</strong>
              <time>{}</time>
            </span>
            <span class="channel-preview">{preview_line}</span>
          </span>
          {}
        </button>"#,
            if chat.jid == req.active { "active" } else { "" },
            escape(&chat.jid),
            avatar(&chat.name, &chat.jid, "avatar", &req.photos),
            escape(&chat.name),
            stamp_of(stamp, req.now),
            if chat.unread > 0 {
                format!(r#"<span class="badge">{}</span>"#, chat.unread)
            } else {
                String::new()
            },
        ));
    }

    ChannelsOutput {
        empty_hidden: !chats.is_empty(),
        html,
        queue: chats.iter().take(20).map(|chat| chat.jid.clone()).collect(),
    }
}

/* ---------------------------------------------------------------- community */

#[derive(Debug, Serialize)]
pub struct CommunityOutput {
    pub title: String,
    pub html: String,
}

/// The community screen: the announcement subgroup (always labelled
/// "Announcements"), an "Add group" affordance and the child groups.
pub fn community(req: &SidebarRequest, jid: &str) -> CommunityOutput {
    let Some(parent) = req.chats.iter().find(|chat| chat.jid == jid) else {
        return CommunityOutput {
            title: String::new(),
            html: String::new(),
        };
    };

    let mut children: Vec<&SidebarChat> = Vec::new();
    let mut announce: Option<&SidebarChat> = None;
    for other in &req.chats {
        if other.jid == jid {
            continue;
        }
        let Some(meta): Option<&GroupMeta> = req.group_meta.get(&other.jid) else {
            continue;
        };
        if meta.parent_jid.as_deref() != Some(jid) {
            continue;
        }
        if meta.is_announce {
            announce = Some(other);
        } else {
            children.push(other);
        }
    }
    children.sort_by_key(|chat| std::cmp::Reverse(last_ts(chat)));

    let mut html = String::new();
    if let Some(announce) = announce {
        html.push_str(&channel_like_row(announce, req, true, "Announcements"));
    }
    html.push_str(r#"<div class="community-section">Groups you're in</div>"#);
    html.push_str(&format!(
        r#"
        <button type="button" class="chat-item community-add" data-community-add>
          <span class="community-add-badge">{}</span>
          <div class="chat-meta"><div class="chat-top"><strong>Add group</strong></div></div>
        </button>"#,
        icon("add")
    ));
    for child in &children {
        html.push_str(&channel_like_row(child, req, false, ""));
    }
    if children.is_empty() {
        html.push_str(r#"<p class="empty">No groups in this community yet.</p>"#);
    }

    CommunityOutput {
        title: display_name(&parent.name, &parent.jid, req),
        html,
    }
}

/// One chat row for the community screen, with the `community-announce`
/// modifier and a forced display label when needed.
fn channel_like_row(
    chat: &SidebarChat,
    req: &SidebarRequest,
    announce: bool,
    label: &str,
) -> String {
    let last = chat.last.as_ref();
    let stamp = last.map(|m| m.timestamp).unwrap_or(chat.last_ts);
    let preview_text = match last {
        Some(message) => preview(message, chat, req),
        None => "Tap to open".to_string(),
    };
    let unread = if is_self_jid(&chat.jid, req) {
        0
    } else {
        chat.unread
    };
    let name = if label.is_empty() {
        display_name(&chat.name, &chat.jid, req)
    } else {
        label.to_string()
    };
    format!(
        r#"
        <li class="chat-item {} {}" data-jid="{}">
          {}
          <div class="chat-meta">
            <div class="chat-top"><strong>{}</strong><time>{}</time></div>
            <div class="chat-preview"><span class="preview-line"><span class="chat-preview-text">{}</span></span>
              {}
            </div>
          </div>
        </li>"#,
        if announce { "community-announce" } else { "" },
        if unread > 0 { "unread" } else { "" },
        escape(&chat.jid),
        avatar(&chat.name, &chat.jid, "avatar", &req.photos),
        escape(&name),
        stamp_of(stamp, req.now),
        escape(&preview_text),
        if unread > 0 {
            format!(r#"<span class="badge">{unread}</span>"#)
        } else {
            String::new()
        },
    )
}

/* ------------------------------------------------------------------ starred */

#[derive(Debug, Clone, Deserialize)]
pub struct StarredEntry {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub message: MessageRow,
}

#[derive(Debug, Serialize)]
pub struct StarredOutput {
    pub html: String,
}

/// The starred-messages screen, newest first.
pub fn starred(entries: &[StarredEntry], req: &SidebarRequest) -> StarredOutput {
    if entries.is_empty() {
        return StarredOutput {
            html: r#"<p class="empty">No starred messages yet. Right-click a message and pick Star message.</p>"#
                .to_string(),
        };
    }
    let mut rows: Vec<&StarredEntry> = entries.iter().collect();
    rows.sort_by_key(|entry| std::cmp::Reverse(entry.message.timestamp));

    let mut html = String::new();
    for entry in rows {
        let chat = req.chats.iter().find(|chat| chat.jid == entry.jid);
        let is_group = chat.map(|chat| chat.is_group).unwrap_or(false);
        let fallback = SidebarChat {
            jid: entry.jid.clone(),
            ..Default::default()
        };
        let chat_ref = chat.unwrap_or(&fallback);
        let name = display_name(&chat_ref.name, &chat_ref.jid, req);
        let text = preview(&entry.message, chat_ref, req);
        html.push_str(&format!(
            r#"
        <button type="button" class="starred-row" data-starred-jid="{}" data-starred-id="{}">
          <div class="starred-top">
            <strong>{}</strong>
            <time>{}</time>
          </div>
          <div class="starred-text"><span class="starred-icon">{}</span>{}</div>
        </button>"#,
            escape(&entry.jid),
            escape(&entry.message.id),
            escape(&name),
            stamp_of(entry.message.timestamp, req.now),
            icon("star"),
            escape(&text),
        ));
        let _ = is_group;
    }
    StarredOutput { html }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn chat(jid: &str, name: &str, ts: i64) -> SidebarChat {
        SidebarChat {
            jid: jid.to_string(),
            name: name.to_string(),
            last_ts: ts,
            last: Some(MessageRow {
                text: format!("post {name}"),
                timestamp: ts,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn request(chats: Vec<SidebarChat>) -> SidebarRequest {
        SidebarRequest {
            chats,
            contacts: HashMap::new(),
            photos: HashMap::new(),
            group_meta: HashMap::new(),
            phones: HashMap::new(),
            sender_names: HashMap::new(),
            me: "Me".into(),
            me_jid: "62111@s.whatsapp.net".into(),
            me_lid: String::new(),
            self_handle: String::new(),
            active: String::new(),
            search: String::new(),
            filter: "all".into(),
            show_deleted: false,
            selecting: false,
            selected: Vec::new(),
            archived_open: false,
            max_search_rows: 40,
            now: 1_770_000_000_000,
        }
    }

    #[test]
    fn channels_are_newsletters_only() {
        let mut channel = chat("1@newsletter", "News", 3000);
        channel.unread = 2;
        channel.last = Some(MessageRow {
            text: "hello".into(),
            kind: "image".into(),
            timestamp: 3000,
            ..Default::default()
        });
        let mut req = request(vec![channel, chat("2@s.whatsapp.net", "Ada", 4000)]);
        req.active = "1@newsletter".into();
        let out = channels(&req, "");
        assert!(out.html.contains("channel-row active"));
        assert!(out.html.contains("Photo: hello"));
        assert!(out.html.contains("channel-kind"));
        assert!(out.html.contains(r#"<span class="badge">2</span>"#));
        assert!(!out.html.contains("Ada"));
        assert!(out.empty_hidden);

        let out = channels(&req, "nope");
        assert!(!out.empty_hidden);
        assert!(out.html.is_empty());
    }

    #[test]
    fn community_screen_lists_children_and_announcements() {
        let parent = chat("1@g.us", "Community", 1000);
        let mut child = chat("2@g.us", "Child", 5000);
        child.is_group = true;
        let mut announce = chat("3@g.us", "Community", 9000);
        announce.is_group = true;
        let mut req = request(vec![child, announce, parent]);
        req.group_meta.insert(
            "2@g.us".into(),
            GroupMeta {
                parent_jid: Some("1@g.us".into()),
                ..Default::default()
            },
        );
        req.group_meta.insert(
            "3@g.us".into(),
            GroupMeta {
                parent_jid: Some("1@g.us".into()),
                is_announce: true,
                ..Default::default()
            },
        );
        let out = community(&req, "1@g.us");
        assert_eq!(out.title, "Community");
        assert!(out.html.contains("community-announce"));
        assert!(out.html.contains(">Announcements<"));
        assert!(out.html.contains("community-add"));
        assert!(out.html.contains("Groups you&#39;re in") || out.html.contains("Groups you're in"));
        assert!(out.html.contains(">Child<"));
    }

    #[test]
    fn starred_rows_are_newest_first() {
        let mut req = request(vec![chat("2@s.whatsapp.net", "Ada", 4000)]);
        req.active = "2@s.whatsapp.net".into();
        let entries = vec![
            StarredEntry {
                jid: "2@s.whatsapp.net".into(),
                message: MessageRow {
                    id: "old".into(),
                    text: "older".into(),
                    timestamp: 1000,
                    ..Default::default()
                },
            },
            StarredEntry {
                jid: "2@s.whatsapp.net".into(),
                message: MessageRow {
                    id: "new".into(),
                    text: "newer".into(),
                    timestamp: 9000,
                    ..Default::default()
                },
            },
        ];
        let out = starred(&entries, &req);
        assert!(out.html.find("newer").unwrap() < out.html.find("older").unwrap());
        assert!(out.html.contains(r#"data-starred-jid="2@s.whatsapp.net""#));
        assert!(out.html.contains("starred-icon"));

        let empty = starred(&[], &req);
        assert!(empty.html.contains("No starred messages yet"));
    }
}
