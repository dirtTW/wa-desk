//! The chat-list (sidebar) renderer: filtering, pin order, community nesting,
//! search sections and the row markup itself. Input is the webview's chat
//! state; output is ready-to-inject HTML plus the profile photos worth
//! requesting.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::html::{avatar, escape, icon};
use crate::labels::stamp_of;
use crate::text::{
    MentionResolver, MessageRow, chat_subtitle as _chat_subtitle, is_self_jid, preview_of,
    preview_tick, sender_color as _sender_color,
};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GroupMeta {
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub is_community: bool,
    #[serde(default)]
    pub parent_jid: Option<String>,
    #[serde(default)]
    pub is_announce: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SidebarChat {
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub unread: i64,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub pin_ts: i64,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub cleared: bool,
    #[serde(default)]
    pub mute_until: i64,
    #[serde(default)]
    pub mute_forever: bool,
    #[serde(default)]
    pub last_ts: i64,
    /// The newest few messages, so the renderer can pick the visible one.
    #[serde(default)]
    pub tail: Vec<MessageRow>,
    /// A pre-picked last message (older callers), still honoured.
    #[serde(default)]
    pub last: Option<MessageRow>,
}

impl SidebarChat {
    /// The newest message the eye toggle shows.
    pub fn last_visible(&self, show_deleted: bool) -> Option<&MessageRow> {
        if let Some(last) = self.last.as_ref() {
            return Some(last);
        }
        self.tail
            .iter()
            .rev()
            .find(|message| message.is_visible(show_deleted))
    }

    pub fn last_ts_of(&self) -> i64 {
        self.tail
            .last()
            .map(|message| message.timestamp)
            .or_else(|| self.last.as_ref().map(|message| message.timestamp))
            .unwrap_or(self.last_ts)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SidebarRequest {
    #[serde(default)]
    pub chats: Vec<SidebarChat>,
    #[serde(default)]
    pub contacts: HashMap<String, String>,
    #[serde(default)]
    pub photos: HashMap<String, String>,
    #[serde(default)]
    pub group_meta: HashMap<String, GroupMeta>,
    #[serde(default)]
    pub phones: HashMap<String, String>,
    /// user part -> last push name seen in the open chat.
    #[serde(default)]
    pub sender_names: HashMap<String, String>,
    #[serde(default)]
    pub me: String,
    #[serde(default)]
    pub me_jid: String,
    #[serde(default)]
    pub me_lid: String,
    #[serde(default)]
    pub self_handle: String,
    #[serde(default)]
    pub active: String,
    #[serde(default)]
    pub search: String,
    #[serde(default = "default_filter")]
    pub filter: String,
    #[serde(default)]
    pub show_deleted: bool,
    #[serde(default)]
    pub selecting: bool,
    #[serde(default)]
    pub selected: Vec<String>,
    #[serde(default)]
    pub archived_open: bool,
    #[serde(default = "default_search_rows")]
    pub max_search_rows: usize,
    #[serde(default)]
    pub now: i64,
}

fn default_filter() -> String {
    "all".to_string()
}

fn default_search_rows() -> usize {
    40
}

#[derive(Debug, Serialize)]
pub struct SidebarOutput {
    pub html: String,
    pub empty_text: String,
    pub empty_hidden: bool,
    pub queue: Vec<String>,
}

fn is_channel(jid: &str) -> bool {
    jid.ends_with("@newsletter")
}

fn is_group_jid(jid: &str) -> bool {
    jid.ends_with("@g.us")
}

fn self_chat_display(name: &str, jid: &str, req: &SidebarRequest) -> String {
    let name = if name.is_empty() {
        jid.split('@').next().unwrap_or("").to_string()
    } else {
        name.to_string()
    };
    if is_self_jid(jid, &req.me_jid, &req.me_lid) && !name.to_lowercase().ends_with("(you)") {
        return format!("{name} (You)");
    }
    name
}

fn muted(chat: &SidebarChat, now: i64) -> bool {
    chat.mute_forever || chat.mute_until > now
}

/// The preview line, with mention ids replaced by display names.
fn preview_text(message: &MessageRow, chat: &SidebarChat, req: &SidebarRequest) -> String {
    let resolver = MentionResolver {
        me: Some(req.me.clone()),
        me_jid: Some(req.me_jid.clone()),
        me_lid: Some(req.me_lid.clone()),
        contacts: &req.contacts,
        phones: &req.phones,
        sender_names: &req.sender_names,
        self_handle: Some(req.self_handle.clone()),
    };
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
    let text = preview_of(message, chat.is_group, req.show_deleted, &contact_name);
    resolver.rewrite_preview_mentions(&text)
}

fn highlight(text: &str, needle: &str) -> String {
    crate::text::highlight_match(text, needle)
}

fn row_html(
    chat: &SidebarChat,
    needle: &str,
    req: &SidebarRequest,
    nested: bool,
    active: bool,
) -> String {
    let last = chat.last_visible(req.show_deleted);
    let stamp = last.map(|m| m.timestamp).unwrap_or(chat.last_ts);
    let preview = match last {
        Some(message) => preview_text(message, chat, req),
        None => "Tap to open".to_string(),
    };
    let tick = last
        .map(|m| preview_tick(m, chat.is_group))
        .unwrap_or_default();
    let unread = if is_self_jid(&chat.jid, &req.me_jid, &req.me_lid) {
        0
    } else {
        chat.unread
    };
    let checked = req.selecting && req.selected.iter().any(|jid| jid == &chat.jid);
    let classes = format!(
        "chat-item{} {} {} {} {} {}",
        if nested { " nested" } else { "" },
        if active { "active" } else { "" },
        if unread > 0 { "unread" } else { "" },
        if chat.deleted { "deleted" } else { "" },
        if req.selecting { "selectable" } else { "" },
        if checked { "selected" } else { "" },
    );
    let classes = classes.trim().to_string();

    let check = if req.selecting {
        format!(
            r#"<span class="chat-check">{}</span>"#,
            if checked {
                icon("check")
            } else {
                String::new()
            }
        )
    } else {
        String::new()
    };
    let mute = if muted(chat, req.now) {
        r#"<span class="chat-muteglyph" title="Muted"><svg viewBox="0 -960 960 960" aria-hidden="true"><path d="M160-200v-80h80v-280q0-33 8.5-65t25.5-61l60 60q-7 16-10.5 32.5T320-560v280h248L56-792l56-56 736 736-56 56-146-144H160Zm560-154-80-80v-126q0-66-47-113t-113-47q-26 0-50 8t-44 24l-58-58q20-16 43-28t49-18v-28q0-25 17.5-42.5T480-880q25 0 42.5 17.5T540-820v28q80 20 130 84.5T720-560v206Zm-276-50Zm36 324q-33 0-56.5-23.5T400-160h160q0 33-23.5 56.5T480-80Zm33-481Z"/></svg></span>"#.to_string()
    } else {
        String::new()
    };
    let pin = if chat.pinned {
        r#"<span class="chat-pin" title="Pinned"><svg viewBox="0 -960 960 960" aria-hidden="true"><path d="m640-480 80 80v80H520v240l-40 40-40-40v-240H240v-80l80-80v-280h-40v-80h400v80h-40v280Zm-286 80h252l-46-46v-314H400v314l-46 46Zm126 0Z"/></svg></span>"#.to_string()
    } else {
        String::new()
    };
    let badge = if unread > 0 {
        format!(r#"<span class="badge">{unread}</span>"#)
    } else {
        String::new()
    };
    let name = self_chat_display(&chat.name, &chat.jid, req);

    format!(
        r#"
        <li class="{classes}" data-jid="{}">
          {check}
          {}
          <div class="chat-meta">
            <div class="chat-top">
              <strong>{}</strong>
              {mute}
              {pin}
              <time>{}</time>
            </div>
            <div class="chat-preview"><span class="preview-line">{tick}<span class="chat-preview-text">{}</span></span>
              {badge}
            </div>
          </div>
        </li>"#,
        escape(&chat.jid),
        avatar(&chat.name, &chat.jid, "avatar", &req.photos),
        highlight(&name, needle),
        stamp_of(stamp, req.now),
        highlight(&preview, needle),
    )
}

fn community_row(unit: &CommunityUnit, req: &SidebarRequest) -> String {
    let label = if unit.render.is_empty() {
        "Community".to_string()
    } else {
        format!(
            "Community \u{b7} {} group{}",
            unit.render.len(),
            if unit.render.len() == 1 { "" } else { "s" }
        )
    };
    let ts = unit
        .children
        .iter()
        .map(|child| {
            child
                .last
                .as_ref()
                .map(|m| m.timestamp)
                .unwrap_or(child.last_ts)
        })
        .max()
        .unwrap_or(0);
    let name = self_chat_display(&unit.chat.name, &unit.chat.jid, req);
    let mut html = format!(
        r#"
        <li class="chat-item community" data-jid="{}">
          {}
          <div class="chat-meta">
            <div class="chat-top"><strong>{}</strong>{}</div>
            <div class="chat-preview"><span class="preview-line"><span class="chat-preview-text">{}</span></span></div>
          </div>
        </li>"#,
        escape(&unit.chat.jid),
        avatar(&unit.chat.name, &unit.chat.jid, "avatar", &req.photos),
        escape(&name),
        if ts > 0 {
            format!("<time>{}</time>", stamp_of(ts, req.now))
        } else {
            String::new()
        },
        escape(&label),
    );
    for child in &unit.render {
        html.push_str(&row_html(child, "", req, true, child.jid == req.active));
    }
    html
}

struct CommunityUnit<'a> {
    chat: &'a SidebarChat,
    children: Vec<&'a SidebarChat>,
    render: Vec<&'a SidebarChat>,
}

fn contact_row(jid: &str, name: &str, needle: &str, req: &SidebarRequest) -> String {
    format!(
        r#"
        <li class="chat-item search-contact" data-picker-jid="{}" data-picker-name="{}">
          {}
          <div class="chat-meta">
            <div class="chat-top"><strong>{}</strong></div>
            <div class="chat-preview"><span>{}</span></div>
          </div>
        </li>"#,
        escape(jid),
        escape(name),
        avatar(name, jid, "avatar", &req.photos),
        highlight(name, needle),
        escape(&crate::text::format_phone(&crate::text::phone_from_jid(
            jid
        ))),
    )
}

/// Render the sidebar. Returns the list HTML, the empty-state text and the
/// chats whose profile photos are worth fetching.
pub fn render(req: &SidebarRequest) -> SidebarOutput {
    let needle = req.search.trim().to_lowercase();
    let searching = !needle.is_empty();

    let visible: Vec<&SidebarChat> = req
        .chats
        .iter()
        .filter(|chat| !is_channel(&chat.jid))
        .filter(|chat| req.show_deleted || !chat.deleted)
        .filter(|chat| {
            if searching {
                return true;
            }
            // Archived chats live in their own view.
            let archived_view = req.archived_open;
            if chat.archived != archived_view {
                return false;
            }
            match req.filter.as_str() {
                "unread" => chat.unread != 0,
                "groups" => chat.is_group,
                _ => true,
            }
        })
        .filter(|chat| {
            if !searching {
                return true;
            }
            match req.filter.as_str() {
                "unread" => chat.unread != 0,
                "groups" => chat.is_group,
                _ => true,
            }
        })
        .collect();

    let archived_count = req
        .chats
        .iter()
        .filter(|chat| {
            !is_channel(&chat.jid) && chat.archived && (req.show_deleted || !chat.deleted)
        })
        .count();

    if searching {
        return render_search(&visible, &needle, archived_count, req);
    }

    let mut sorted = visible.clone();
    sorted.sort_by(|a, b| {
        let pa = if a.pinned { 1 } else { 0 };
        let pb = if b.pinned { 1 } else { 0 };
        if pa != pb {
            return pb.cmp(&pa);
        }
        if pa == 1 {
            return a.pin_ts.cmp(&b.pin_ts);
        }
        b.last_ts_of().cmp(&a.last_ts_of())
    });

    let archived = if archived_count > 0 || req.archived_open {
        format!(
            r#"
        <button type="button" class="archived-row" data-archived-toggle>
          <span class="archived-arrow">{}</span>
          Archived
          {}
        </button>"#,
            icon(if req.archived_open {
                "expand_more"
            } else {
                "chevron_right"
            }),
            if archived_count > 0 {
                format!(r#"<span class="archived-count">{archived_count}</span>"#)
            } else {
                String::new()
            }
        )
    } else {
        String::new()
    };

    let mut communities: Vec<CommunityUnit> = Vec::new();
    let mut by_parent: HashMap<String, usize> = HashMap::new();
    for chat in &sorted {
        if let Some(meta) = req.group_meta.get(&chat.jid)
            && meta.is_community
        {
            by_parent.insert(chat.jid.clone(), communities.len());
            communities.push(CommunityUnit {
                chat,
                children: Vec::new(),
                render: Vec::new(),
            });
        }
    }
    let mut top: Vec<&SidebarChat> = Vec::new();
    for chat in &sorted {
        let meta = req.group_meta.get(&chat.jid);
        if meta.map(|m| m.is_community).unwrap_or(false) {
            continue;
        }
        let parent = meta
            .and_then(|m| m.parent_jid.clone())
            .and_then(|parent| by_parent.get(&parent).copied());
        if let Some(index) = parent {
            let unit = &mut communities[index];
            unit.children.push(chat);
            if !meta.map(|m| m.is_announce).unwrap_or(false) {
                unit.render.push(chat);
            }
            continue;
        }
        if !meta.map(|m| m.is_announce).unwrap_or(false) {
            top.push(chat);
        }
    }
    let nested: HashSet<&str> = communities
        .iter()
        .flat_map(|unit| unit.children.iter().map(|child| child.jid.as_str()))
        .collect();
    let top: Vec<&&SidebarChat> = top
        .iter()
        .filter(|chat| !nested.contains(chat.jid.as_str()))
        .collect();

    enum Unit<'a> {
        Chat(&'a SidebarChat),
        Community(usize),
    }
    let mut units: Vec<(i64, Unit)> = top
        .iter()
        .map(|chat| {
            (
                if chat.pinned {
                    i64::MAX
                } else {
                    chat.last
                        .as_ref()
                        .map(|m| m.timestamp)
                        .unwrap_or(chat.last_ts)
                },
                Unit::Chat(chat),
            )
        })
        .collect();
    for (index, unit) in communities.iter().enumerate() {
        let child_ts = unit
            .children
            .iter()
            .map(|child| {
                child
                    .last
                    .as_ref()
                    .map(|m| m.timestamp)
                    .unwrap_or(child.last_ts)
            })
            .max()
            .unwrap_or(0);
        let own_ts = unit
            .chat
            .last
            .as_ref()
            .map(|m| m.timestamp)
            .unwrap_or(unit.chat.last_ts);
        units.push((own_ts.max(child_ts), Unit::Community(index)));
    }
    units.sort_by_key(|a| std::cmp::Reverse(a.0));

    let mut html = archived;
    for (_, unit) in &units {
        match unit {
            Unit::Chat(chat) => {
                html.push_str(&row_html(chat, "", req, false, chat.jid == req.active))
            }
            Unit::Community(index) => html.push_str(&community_row(&communities[*index], req)),
        }
    }

    let mut queue: Vec<String> = sorted
        .iter()
        .take(30)
        .map(|chat| chat.jid.clone())
        .collect();
    for unit in &communities {
        for child in &unit.children {
            queue.push(child.jid.clone());
        }
    }

    let empty_text = if req.filter == "unread" {
        "No unread chats.".to_string()
    } else if req.filter == "groups" {
        "No groups yet.".to_string()
    } else {
        "No chats yet. Link your phone, or wait for a message to arrive.".to_string()
    };

    SidebarOutput {
        empty_hidden: !sorted.is_empty(),
        empty_text,
        queue,
        html,
    }
}

fn render_search(
    visible: &[&SidebarChat],
    needle: &str,
    _archived: usize,
    req: &SidebarRequest,
) -> SidebarOutput {
    let limit = req.max_search_rows.max(1);
    let mut chats: Vec<(&SidebarChat, String)> = Vec::new();
    let mut groups: Vec<(&SidebarChat, String)> = Vec::new();
    let mut shown: HashSet<String> = HashSet::new();

    for chat in visible {
        let label = self_chat_display(&chat.name, &chat.jid, req);
        if chat.name.to_lowercase().contains(needle) || label.to_lowercase().contains(needle) {
            chats.push((chat, label));
            shown.insert(chat.jid.clone());
            continue;
        }
        if !chat.is_group {
            continue;
        }
        if let Some(member) = chat
            .last_visible(req.show_deleted)
            .filter(|message| message.sender_name.to_lowercase().contains(needle))
        {
            groups.push((chat, member.sender_name.clone()));
        }
    }
    chats.sort_by_key(|a| std::cmp::Reverse(a.0.last_ts_of()));
    groups.sort_by_key(|a| std::cmp::Reverse(a.0.last_ts_of()));

    let self_phone = crate::text::phone_from_jid(&req.me_jid);
    let mut contacts: Vec<(String, String)> = req
        .contacts
        .iter()
        .filter(|(key, name)| {
            !name.is_empty()
                && key.chars().all(|ch| ch.is_ascii_digit())
                && (8..=13).contains(&key.len())
                && *key != &self_phone
                && name.to_lowercase().contains(needle)
        })
        .map(|(key, name)| (format!("{key}@s.whatsapp.net"), name.clone()))
        .filter(|(jid, _)| !shown.contains(jid))
        .collect();
    contacts.sort_by(|a, b| a.1.cmp(&b.1));

    let mut parts: Vec<String> = Vec::new();
    if !chats.is_empty() {
        parts.push(r#"<div class="search-section">Chats</div>"#.to_string());
        for (chat, _) in chats.iter().take(limit) {
            parts.push(row_html(chat, needle, req, false, chat.jid == req.active));
        }
    }
    if !contacts.is_empty() {
        parts.push(r#"<div class="search-section">Contacts</div>"#.to_string());
        for (jid, name) in contacts.iter().take(limit) {
            parts.push(contact_row(jid, name, needle, req));
        }
    }
    if !groups.is_empty() {
        parts.push(r#"<div class="search-section">Groups in common</div>"#.to_string());
        for (chat, member) in groups.iter().take(limit) {
            parts.push(format!(
                r#"
        <li class="chat-item" data-jid="{}">
          {}
          <div class="chat-meta">
            <div class="chat-top"><strong>{}</strong></div>
            <div class="chat-preview"><span>{} is also in this group</span></div>
          </div>
        </li>"#,
                escape(&chat.jid),
                avatar(&chat.name, &chat.jid, "avatar", &req.photos),
                highlight(&chat.name, needle),
                highlight(member, needle),
            ));
        }
    }

    let mut queue: Vec<String> = chats.iter().take(20).map(|(c, _)| c.jid.clone()).collect();
    queue.extend(contacts.iter().take(20).map(|(jid, _)| jid.clone()));
    queue.extend(groups.iter().take(20).map(|(c, _)| c.jid.clone()));

    SidebarOutput {
        empty_hidden: !parts.is_empty(),
        empty_text: format!("No chats match \"{}\".", req.search),
        html: parts.join(""),
        queue,
    }
}

/// Unused helpers kept so the renderer can grow without re-deriving them.
pub fn subtitle(jid: &str, is_group: bool, me_jid: &str, me_lid: &str, phone: &str) -> String {
    _chat_subtitle(jid, is_group, me_jid, me_lid, phone)
}

pub fn color(jid: &str) -> &'static str {
    _sender_color(jid)
}

pub fn is_group(jid: &str) -> bool {
    is_group_jid(jid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(jid: &str, name: &str, ts: i64) -> SidebarChat {
        SidebarChat {
            jid: jid.to_string(),
            name: name.to_string(),
            last_ts: ts,
            last: Some(MessageRow {
                text: format!("hello {name}"),
                timestamp: ts,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn request(chats: Vec<SidebarChat>) -> SidebarRequest {
        SidebarRequest {
            chats,
            me: "Me".to_string(),
            me_jid: "62111@s.whatsapp.net".to_string(),
            me_lid: String::new(),
            self_handle: "@me".to_string(),
            active: String::new(),
            search: String::new(),
            filter: "all".to_string(),
            show_deleted: false,
            selecting: false,
            selected: Vec::new(),
            archived_open: false,
            max_search_rows: 40,
            now: 1_770_000_000_000,
            contacts: HashMap::new(),
            photos: HashMap::new(),
            group_meta: HashMap::new(),
            phones: HashMap::new(),
            sender_names: HashMap::new(),
        }
    }

    #[test]
    fn rows_carry_the_official_markup() {
        let out = render(&request(vec![chat(
            "6285@s.whatsapp.net",
            "Ada",
            1_770_000_000_000,
        )]));
        assert!(out.html.contains("class=\"chat-item"));
        assert!(out.html.contains(r#"data-jid="6285@s.whatsapp.net""#));
        assert!(out.html.contains("<strong>Ada</strong>"));
        assert!(out.html.contains("hello Ada"));
        assert!(
            out.empty_hidden,
            "the empty state is hidden while chats exist"
        );
        assert_eq!(out.queue, vec!["6285@s.whatsapp.net".to_string()]);
    }

    #[test]
    fn pinned_chats_sort_first_and_by_pin_order() {
        let mut a = chat("a@s.whatsapp.net", "A", 5000);
        a.pinned = true;
        a.pin_ts = 2;
        let mut b = chat("b@s.whatsapp.net", "B", 1000);
        b.pinned = true;
        b.pin_ts = 1;
        let c = chat("c@s.whatsapp.net", "C", 9000);
        let out = render(&request(vec![c, a, b]));
        let pos = |needle: &str| out.html.find(needle).unwrap();
        assert!(pos("data-jid=\"b@") < pos("data-jid=\"a@"));
        assert!(pos("data-jid=\"a@") < pos("data-jid=\"c@"));
    }

    #[test]
    fn archived_chats_live_behind_the_toggle() {
        let mut archived = chat("a@s.whatsapp.net", "Old", 5000);
        archived.archived = true;
        let out = render(&request(vec![
            archived,
            chat("b@s.whatsapp.net", "New", 6000),
        ]));
        assert!(out.html.contains("archived-row"));
        assert!(out.html.contains(">Archived<") || out.html.contains("Archived"));
        assert!(
            !out.html.contains("data-jid=\"a@"),
            "archived row is hidden"
        );
        assert!(out.html.contains("data-jid=\"b@"));

        let mut req = request(vec![]);
        let mut archived = chat("a@s.whatsapp.net", "Old", 5000);
        archived.archived = true;
        req.chats = vec![archived];
        req.archived_open = true;
        assert!(render(&req).html.contains("data-jid=\"a@"));
    }

    #[test]
    fn unread_filter_and_self_chat() {
        let mut unread = chat("a@s.whatsapp.net", "A", 5000);
        unread.unread = 3;
        let read = chat("b@s.whatsapp.net", "B", 4000);
        let mut req = request(vec![unread, read]);
        req.filter = "unread".to_string();
        let out = render(&req);
        assert!(out.html.contains("data-jid=\"a@"));
        assert!(!out.html.contains("data-jid=\"b@"));

        // The self chat never shows unread and is labelled "(You)".
        let mut own = chat("62111@s.whatsapp.net", "Me", 7000);
        own.unread = 4;
        let out = render(&request(vec![own]));
        assert!(out.html.contains("Me (You)"));
        assert!(!out.html.contains("class=\"badge\""));
    }

    #[test]
    fn muted_and_pinned_glyphs() {
        let mut chat = chat("a@s.whatsapp.net", "A", 5000);
        chat.mute_forever = true;
        chat.pinned = true;
        let out = render(&request(vec![chat]));
        assert!(out.html.contains("chat-muteglyph"));
        assert!(out.html.contains("chat-pin"));
    }

    #[test]
    fn search_splits_into_sections() {
        let mut req = request(vec![
            chat("6285@s.whatsapp.net", "Ada Lovelace", 5000),
            chat("1@g.us", "Team", 4000),
        ]);
        req.search = "ada".to_string();
        let out = render(&req);
        assert!(out.html.contains("search-section\">Chats"));
        assert!(out.html.contains("<mark class=\"search-hit\">Ada</mark>"));

        let mut req = request(vec![]);
        req.search = "budi".to_string();
        req.contacts
            .insert("628770000".to_string(), "Budi".to_string());
        let out = render(&req);
        assert!(out.html.contains("search-section\">Contacts"));
        assert!(
            out.html
                .contains(r#"data-picker-jid="628770000@s.whatsapp.net""#)
        );
        assert!(out.html.contains("+62 877"));

        let mut req = request(vec![]);
        req.search = "zzz".to_string();
        let out = render(&req);
        assert!(!out.empty_hidden);
        assert_eq!(out.empty_text, "No chats match \"zzz\".");
    }

    #[test]
    fn communities_nest_their_groups() {
        let parent = chat("1@g.us", "Community", 1000);
        let mut child = chat("2@g.us", "Child", 5000);
        child.is_group = true;
        let mut announce = chat("3@g.us", "Announce", 9000);
        announce.is_group = true;
        let mut req = request(vec![child, announce, parent]);
        req.group_meta.insert(
            "1@g.us".to_string(),
            GroupMeta {
                is_community: true,
                ..Default::default()
            },
        );
        req.group_meta.insert(
            "2@g.us".to_string(),
            GroupMeta {
                parent_jid: Some("1@g.us".to_string()),
                ..Default::default()
            },
        );
        req.group_meta.insert(
            "3@g.us".to_string(),
            GroupMeta {
                parent_jid: Some("1@g.us".to_string()),
                is_announce: true,
                ..Default::default()
            },
        );
        let out = render(&req);
        assert!(out.html.contains(r#"class="chat-item community""#));
        assert!(out.html.contains("Community \u{b7} 1 group"));
        assert!(out.html.contains("chat-item nested"));
        assert!(
            !out.html.contains("data-jid=\"3@g.us\""),
            "announce group is folded away"
        );
    }

    #[test]
    fn selecting_shows_checkboxes() {
        let mut req = request(vec![chat("a@s.whatsapp.net", "A", 5000)]);
        req.selecting = true;
        req.selected = vec!["a@s.whatsapp.net".to_string()];
        let out = render(&req);
        assert!(out.html.contains("chat-check"));
        assert!(out.html.contains("selected"));
    }

    #[test]
    fn previews_use_names_for_mentions() {
        let mut chat = chat("1@g.us", "Team", 5000);
        chat.is_group = true;
        chat.last = Some(MessageRow {
            text: "hey @62877".to_string(),
            timestamp: 5000,
            sender: "62877@s.whatsapp.net".to_string(),
            sender_name: "Budi".to_string(),
            ..Default::default()
        });
        let mut req = request(vec![chat]);
        req.contacts.insert("62877".to_string(), "Budi".to_string());
        let out = render(&req);
        assert!(out.html.contains("Budi: hey @Budi"));
    }
}
