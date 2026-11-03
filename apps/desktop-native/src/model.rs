//! The native front end's state: chats, messages and what the window shows.
//!
//! Everything user-visible that is *not* painting comes from `wa-ui`, so the
//! native window and the WebView cannot disagree about a timestamp, a preview or
//! a delivery tick.

use std::collections::BTreeMap;

use wa_types::{MessageView, UiEvent};
use wa_ui::labels::stamp_of;
use wa_ui::text::{MessageRow, preview_of};

/// Work the UI asks the session to do.
#[derive(Debug, Clone)]
pub enum Command {
    SendText { chat: String, text: String },
    MarkRead { chat: String },
    Typing { chat: String, typing: bool },
}

#[derive(Debug, Default, Clone)]
pub struct ChatRow {
    pub jid: String,
    pub name: String,
    pub messages: Vec<MessageView>,
    pub unread: i64,
    pub last_ts: i64,
}

#[derive(Debug, Default)]
pub struct Model {
    pub chats: BTreeMap<String, ChatRow>,
    pub active: Option<String>,
    /// The connection line under the title.
    pub status: String,
    /// `Some` while the phone still has to scan something.
    pub qr_svg: Option<String>,
    pub pair_code: Option<String>,
    pub search: String,
    pub composer: String,
    pub now: i64,
}

impl Model {
    pub fn new() -> Self {
        Self {
            status: "Connecting…".to_string(),
            ..Default::default()
        }
    }

    pub fn apply(&mut self, event: &UiEvent) {
        match event {
            UiEvent::Qr { svg, .. } => {
                self.qr_svg = Some(svg.clone());
                self.status = "Scan the QR code with WhatsApp".to_string();
            }
            UiEvent::QrExpired => {
                self.qr_svg = None;
                self.status = "QR expired, waiting for a new one".to_string();
            }
            UiEvent::PairCode { code } => {
                self.pair_code = Some(code.clone());
                self.status = "Enter this code on your phone".to_string();
            }
            UiEvent::Paired { name, .. } => {
                self.qr_svg = None;
                self.pair_code = None;
                self.status = format!("Linked as {name}");
            }
            UiEvent::Connected => self.status = "Connected".to_string(),
            UiEvent::Disconnected { reason } => {
                self.status = format!("Reconnecting… ({reason})");
            }
            UiEvent::LoggedOut { .. } => {
                self.status = "Logged out".to_string();
                self.chats.clear();
                self.active = None;
            }
            UiEvent::PairError { message } => {
                self.status = message.clone();
            }
            UiEvent::Chat {
                jid: chat, name, ..
            } => {
                let row = self.chats.entry(chat.clone()).or_insert_with(|| ChatRow {
                    jid: chat.clone(),
                    ..Default::default()
                });
                if !name.is_empty() {
                    row.name = name.clone();
                }
            }
            UiEvent::ChatName { jid, name } | UiEvent::ContactName { jid, name } => {
                if let Some(row) = self.chats.get_mut(jid) {
                    row.name = name.clone();
                }
            }
            UiEvent::Message(message) | UiEvent::HistoryMessage(message) => {
                self.upsert(message.clone());
            }
            UiEvent::Sent {
                chat,
                id,
                timestamp,
            } => {
                if let Some(last) = self
                    .chats
                    .get_mut(chat)
                    .and_then(|row| row.messages.last_mut())
                    && last.from_me
                    && last.id.starts_with("local-")
                {
                    last.id = id.clone();
                    last.timestamp = *timestamp;
                }
            }
            UiEvent::ChatReadMark { chat, read } => {
                if let Some(row) = self.chats.get_mut(chat)
                    && *read
                {
                    row.unread = 0;
                }
            }
            UiEvent::ChatDeleted { chat } if self.active.as_deref() == Some(chat.as_str()) => {
                self.active = None;
            }
            _ => {}
        }
    }

    fn upsert(&mut self, message: MessageView) {
        let chat = message.chat.clone();
        let name = message.chat_name.clone();
        let unread_room = message.from_me;
        let timestamp = message.timestamp;
        let row = self.chats.entry(chat.clone()).or_insert_with(|| ChatRow {
            jid: chat,
            name: name.clone(),
            ..Default::default()
        });
        if !name.is_empty() && row.name.is_empty() {
            row.name = name;
        }
        if let Some(existing) = row.messages.iter_mut().find(|m| m.id == message.id) {
            *existing = message;
        } else {
            row.messages.push(message);
            row.messages.sort_by_key(|m| m.timestamp);
        }
        row.last_ts = row.last_ts.max(timestamp);
        if !unread_room && self.active.as_deref() != Some(row.jid.as_str()) {
            row.unread += 1;
        }
    }

    /// Chat rows for the sidebar, filtered by the search box.
    pub fn visible_chats(&self) -> Vec<&ChatRow> {
        let needle = self.search.trim().to_lowercase();
        let mut rows: Vec<&ChatRow> = self
            .chats
            .values()
            .filter(|row| !row.messages.is_empty() || !row.name.is_empty())
            .filter(|row| {
                needle.is_empty()
                    || row.name.to_lowercase().contains(&needle)
                    || row.jid.to_lowercase().contains(&needle)
            })
            .collect();
        rows.sort_by_key(|row| std::cmp::Reverse(row.last_ts));
        rows
    }

    /// Open a chat: it becomes active and its badge clears.
    pub fn open_chat(&mut self, jid: &str) {
        self.active = Some(jid.to_string());
        if let Some(row) = self.chats.get_mut(jid) {
            row.unread = 0;
        }
    }

    pub fn active_row(&self) -> Option<&ChatRow> {
        self.active.as_ref().and_then(|jid| self.chats.get(jid))
    }

    /// The sidebar subtitle: the newest message preview, from `wa-ui`.
    pub fn row_preview(&self, row: &ChatRow) -> String {
        row.messages
            .last()
            .map(|message| {
                preview_of(
                    &message_row(message),
                    row.jid.ends_with("@g.us"),
                    false,
                    &|_| String::new(),
                )
            })
            .unwrap_or_else(|| "No messages yet".to_string())
    }

    /// The clock beside the row (`8:44 pm`, a date, or a day label).
    pub fn row_stamp(&self, row: &ChatRow) -> String {
        if row.last_ts == 0 {
            return String::new();
        }
        stamp_of(row.last_ts, self.now)
    }

    /// The bubble's own timestamp.
    pub fn bubble_stamp(&self, message: &MessageView) -> String {
        stamp_of(message.timestamp, self.now)
    }

    /// The chat title, unread badge included in the sidebar painting.
    pub fn title(&self, row: &ChatRow) -> String {
        if row.name.is_empty() {
            row.jid.split('@').next().unwrap_or(&row.jid).to_string()
        } else {
            row.name.clone()
        }
    }
}

pub fn message_row(message: &MessageView) -> MessageRow {
    MessageRow {
        id: message.id.clone(),
        text: message.text.clone(),
        kind: message.kind.clone(),
        from_me: message.from_me,
        sender: message.sender.clone().unwrap_or_default(),
        sender_name: message.sender_name.clone().unwrap_or_default(),
        sender_phone: message.sender_phone.clone().unwrap_or_default(),
        status: String::new(),
        ..Default::default()
    }
}

pub fn store_path() -> String {
    let base = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|_| {
            std::env::var("HOME").map(|home| std::path::PathBuf::from(home).join(".local/share"))
        })
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let dir = base.join("io.github.dirttw.wadesk-native");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("whatsapp.db").to_string_lossy().into_owned()
}

pub fn media_dir() -> String {
    let base = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|_| {
            std::env::var("HOME").map(|home| std::path::PathBuf::from(home).join(".local/share"))
        })
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let dir = base.join("io.github.dirttw.wadesk-native").join("media");
    let _ = std::fs::create_dir_all(&dir);
    dir.to_string_lossy().into_owned()
}

/// Headless verification: feed the model the same events the phone would send
/// and check what the window would paint.
pub fn selftest() -> Result<String, String> {
    let mut model = Model::new();
    let now = 1_770_000_000_000;
    model.now = now;

    model.apply(&UiEvent::Connected);
    model.apply(&UiEvent::Message(view(
        "1@s.whatsapp.net",
        "Ada",
        "m1",
        "hello",
        now,
        false,
    )));
    model.apply(&UiEvent::Message(view(
        "1@s.whatsapp.net",
        "Ada",
        "m2",
        "ok",
        now + 1_000,
        true,
    )));
    model.apply(&UiEvent::Message(view(
        "2@g.us",
        "Team",
        "g1",
        "yo",
        now + 2_000,
        false,
    )));

    let rows: Vec<ChatRow> = model.visible_chats().into_iter().cloned().collect();
    if rows.len() != 2 {
        return Err(format!("expected 2 chats, got {}", rows.len()));
    }
    let first = model.title(&rows[0]);
    if first != "Team" {
        return Err(format!("newest chat first, got {first}"));
    }
    if rows[1].unread != 1 {
        return Err(format!("unread should be 1, got {}", rows[1].unread));
    }
    let preview = model.row_preview(&rows[1]);
    if preview != "ok" {
        return Err(format!("preview should be 'ok', got {preview:?}"));
    }
    let stamp = model.row_stamp(&rows[1]);
    if !stamp.contains("pm") && !stamp.contains("am") {
        return Err(format!("stamp should be a clock, got {stamp:?}"));
    }

    // Opening a chat clears its badge and the composer becomes active.
    model.open_chat("1@s.whatsapp.net");
    model.apply(&UiEvent::Message(view(
        "1@s.whatsapp.net",
        "Ada",
        "m3",
        "again",
        now + 3_000,
        false,
    )));
    if model.active_row().map(|row| row.unread) != Some(0) {
        return Err("the open chat must not gain an unread badge".to_string());
    }

    // Search filters by name.
    model.search = "tea".to_string();
    if model.visible_chats().len() != 1 {
        return Err("search should match only Team".to_string());
    }
    model.search.clear();

    Ok(serde_json::json!({
        "chats": model.chats.len(),
        "first": first,
        "preview": preview,
        "stamp": stamp,
        "status": model.status,
    })
    .to_string())
}

fn view(chat: &str, name: &str, id: &str, text: &str, ts: i64, from_me: bool) -> MessageView {
    MessageView {
        chat: chat.to_string(),
        chat_name: name.to_string(),
        sender: if from_me {
            None
        } else {
            Some(chat.to_string())
        },
        sender_name: if from_me {
            None
        } else {
            Some(name.to_string())
        },
        sender_phone: None,
        id: id.to_string(),
        from_me,
        is_group: chat.ends_with("@g.us"),
        timestamp: ts,
        kind: "text".to_string(),
        text: text.to_string(),
        mentions: Vec::new(),
        reply_to: None,
        link: None,
        document: None,
        audio: None,
        stats: None,
        forwarded: false,
        once_recovered: false,
        status_quote: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_fill_the_sidebar() {
        let mut model = Model::new();
        let now = 1_770_000_000_000;
        model.now = now;
        model.apply(&UiEvent::Message(view(
            "1@s.whatsapp.net",
            "Ada",
            "m1",
            "hi",
            now,
            false,
        )));
        assert_eq!(model.visible_chats().len(), 1);
        assert_eq!(model.visible_chats()[0].unread, 1);
    }

    #[test]
    fn the_open_chat_does_not_count_as_unread() {
        let mut model = Model::new();
        model.active = Some("1@s.whatsapp.net".into());
        model.apply(&UiEvent::Message(view(
            "1@s.whatsapp.net",
            "Ada",
            "m1",
            "hi",
            1_770_000_000_000,
            false,
        )));
        assert_eq!(model.active_row().unwrap().unread, 0);
    }

    #[test]
    fn the_selftest_passes() {
        let report = selftest().expect("selftest");
        assert!(report.contains("\"chats\":2"));
    }
}
