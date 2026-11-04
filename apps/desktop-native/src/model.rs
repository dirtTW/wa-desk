//! The native front end's state: chats, messages and what the window shows.
//!
//! Everything user-visible that is *not* painting comes from `wa-ui`, so the
//! native window and the WebView cannot disagree about a timestamp, a preview or
//! a delivery tick.

use std::collections::{BTreeMap, HashMap};

use wa_types::{FavoriteStickerView, GroupInfoView, MessageView, StatusUpdate, UiEvent};
use wa_ui::status::{
    StatusEntryView, StatusRequest, StatusUpdateView, ViewerChromeInput, next_position,
    progress_ratio, viewer_chrome, viewer_groups,
};

use crate::login::{LoginEvent, LoginState, fold};
use wa_ui::labels::{format_followers, stamp_of};
use wa_ui::text::{MessageRow, preview_of};

/// Work the UI asks the session to do.
#[derive(Debug, Clone)]
pub enum Command {
    SendText {
        chat: String,
        text: String,
    },
    MarkRead {
        chat: String,
    },
    Typing {
        chat: String,
        typing: bool,
    },
    /// Ask the phone to show a pairing code instead of a QR.
    PairCode {
        phone: String,
    },
    /// Load a group's metadata for the info screen.
    GroupInfo {
        chat: String,
    },
    /// Restart the pairing session for a fresh QR.
    Restart,
    /// Download (or find) one message's media.
    FetchMedia {
        id: String,
    },
    /// Ask the session for the account's favourite stickers.
    FavoriteStickers,
    /// Send a favourite sticker by its app-state id (`fav-…`).
    SendFavoriteSticker {
        chat: String,
        id: String,
    },
    /// Send a sticker the session already has, by message id.
    ForwardSticker {
        chat: String,
        message_id: String,
    },
    /// Send text as a reply to an existing message.
    SendReply {
        chat: String,
        text: String,
        reply_id: String,
        quoted: Option<String>,
    },
    /// React to a message (an empty emoji removes our reaction).
    React {
        chat: String,
        id: String,
        from_me: bool,
        participant: Option<String>,
        emoji: String,
    },
    /// Rewrite one of our own messages.
    EditMessage {
        chat: String,
        id: String,
        text: String,
    },
    /// Delete on this device only.
    DeleteForMe {
        chat: String,
        id: String,
        from_me: bool,
        participant: Option<String>,
    },
    /// Revoke a message for everyone.
    DeleteForEveryone {
        chat: String,
        id: String,
    },
    /// Forward one message into every chosen chat.
    Forward {
        chats: Vec<String>,
        id: String,
    },
    /// Seed the chat list from the address book and request a history batch.
    InitialSync,
    /// Request the next batch of chat history (the phone caps each batch).
    SyncMore,
    /// Fetch a contact's profile photo URL.
    Avatar {
        jid: String,
    },
    /// Ask the linked session for older status updates.
    StatusHistory,
    /// Ask the phone for one chat's latest messages (opening a chat).
    ChatHistory {
        chat: String,
    },
    /// Apply the chat app-state collection (pins/archives/mutes) so groups can
    /// be discovered without a fresh link.
    SyncChatState,
    /// Unlink this device so a fresh pair imports the complete chat list
    /// (WhatsApp only sends groups and old chats to a new link).
    Relink,
    /// Ask the phone for every group the account participates in.
    SyncGroups,
    /// Chat-level actions from the header menu.
    ChatAction {
        chat: String,
        action: String,
    },
    /// The address book, for the new-chat picker.
    Contacts,
    Star {
        chat: String,
        id: String,
        starred: bool,
        from_me: bool,
        participant: Option<String>,
    },
    /// Create a group with these members.
    CreateGroup {
        subject: String,
        members: Vec<String>,
    },
    /// Save a new contact (phone, name).
    NewContact {
        phone: String,
        name: String,
    },
    /// Add/remove/promote/demote a group member.
    GroupMember {
        chat: String,
        member: String,
        action: String,
    },
    /// Leave a group.
    LeaveGroup {
        chat: String,
    },
    /// Send a file from disk as an attachment.
    SendDocument {
        chat: String,
        path: String,
        caption: Option<String>,
    },
}

/// What a message's context menu asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BubbleAction {
    /// Open this message's media in the viewer.
    OpenMedia(String),
    Reply(String),
    React(String, String),
    Edit(String),
    Delete(String),
    Forward(String),
    Star(String),
    /// Start or stop the voice note's playback.
    PlayAudio(String),
}

/// A followed channel and its (read-only) posts.
#[derive(Debug, Default, Clone)]
pub struct ChannelRow {
    pub jid: String,
    pub name: String,
    pub subscribers: u64,
}

/// One author's stories.
#[derive(Debug, Default, Clone)]
pub struct StatusRow {
    pub jid: String,
    pub name: String,
    pub updates: Vec<StatusUpdateView>,
}

/// A row the status panel lists: who posted, how many, and whether any of
/// it is still unseen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusGroupRow {
    pub jid: String,
    pub name: String,
    pub count: usize,
    pub unseen: bool,
}

/// Where a message's media is in the download lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaState {
    Loading,
    /// The cached file `wa-core` handed back.
    Path(String),
    Failed,
}

#[derive(Debug, Default, Clone)]
pub struct ChatRow {
    pub jid: String,
    pub name: String,
    pub messages: Vec<MessageView>,
    pub unread: i64,
    pub last_ts: i64,
    /// The chat is a community (from its group metadata).
    pub community: bool,
}

/// The sidebar's filter pills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatFilter {
    #[default]
    All,
    Groups,
    Communities,
    Unread,
}

#[derive(Debug, Default)]
pub struct Model {
    pub chats: BTreeMap<String, ChatRow>,
    pub active: Option<String>,
    /// The connection line under the title.
    pub status: String,
    /// The open group's metadata, once the session answered.
    pub group: Option<GroupInfoView>,
    /// Why the metadata could not be fetched.
    pub group_error: Option<String>,
    /// Followed channels (`@newsletter`), newest post first when listed.
    pub channels: BTreeMap<String, ChannelRow>,
    /// Stories by author JID.
    pub statuses: BTreeMap<String, StatusRow>,
    /// The viewer's position: `(group, frame)` while a story is open.
    pub viewer: Option<(usize, usize)>,
    /// When the current frame started, for the progress bar.
    pub frame_started: Option<std::time::Instant>,
    /// Downloaded media per message id: the file `wa-core` cached.
    pub media: HashMap<String, MediaState>,
    /// Before the account is linked the window shows the gate instead of chats.
    pub login: LoginState,
    /// The phone number typed into the pairing-code form.
    pub pair_phone: String,
    pub me_jid: String,
    /// The account's own display name, from the pairing event.
    pub me_name: String,
    pub search: String,
    pub composer: String,
    pub now: i64,
    /// The sidebar's active filter pill.
    pub chat_filter: ChatFilter,
    /// In-chat search: the header bar, its query and the hit being shown.
    pub search_open: bool,
    pub chat_search: String,
    pub search_hit: i64,
    /// The emoji picker: whether it is open, its query and the recents.
    pub emoji_open: bool,
    pub emoji_query: String,
    pub emoji_recents: Vec<String>,
    /// The sticker picker and the account's favourite stickers.
    pub stickers_open: bool,
    pub favorites: Vec<FavoriteStickerView>,
    pub favorites_error: Option<String>,
    /// The media viewer's position among the open chat's photos and videos.
    pub media_viewer: Option<usize>,
    /// The message the composer is replying to, and the one being edited.
    pub reply_to: Option<MessageView>,
    pub editing: Option<MessageView>,
    /// The message whose delete dialog is open.
    pub delete_target: Option<MessageView>,
    /// The message being forwarded, its chat search and chosen chats.
    pub forward_target: Option<MessageView>,
    pub forward_search: String,
    pub forward_chosen: Vec<String>,
    /// Reactions per message id: emoji -> (count, ours).
    pub reactions: BTreeMap<String, BTreeMap<String, (usize, bool)>>,
    /// Set once on connect when there is nothing to show, so the app asks the
    /// session for the address book and a batch of history.
    pub needs_history: bool,
    /// Incremented on every `Connected`, so the window can start a new backfill.
    pub connection_epoch: u64,
    /// Starred message ids, across chats.
    pub starred: std::collections::BTreeSet<String>,
}

impl Model {
    pub fn new() -> Self {
        Self {
            status: "Connecting…".to_string(),
            ..Default::default()
        }
    }

    /// A human file size, WhatsApp style: `812 B`, `4.2 kB`, `1.8 MB`.
    pub fn file_size(bytes: u64) -> String {
        const UNITS: [&str; 4] = ["B", "kB", "MB", "GB"];
        let mut value = bytes as f64;
        let mut unit = 0;
        while value >= 1000.0 && unit + 1 < UNITS.len() {
            value /= 1000.0;
            unit += 1;
        }
        if unit == 0 {
            format!("{bytes} B")
        } else {
            format!("{value:.1} {}", UNITS[unit])
        }
    }

    /// Sample data for `--preview`: a group and two chats, so the window can be
    /// inspected (and screenshotted) without a linked session.
    pub fn demo() -> Self {
        let mut model = Model::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(1_770_000_000_000);
        model.now = now;
        model.login = LoginState::Linked;
        model.status = "Connected".to_string();
        model.me_name = "You".to_string();
        model.me_jid = "0@s.whatsapp.net".to_string();

        let mut group = |sender: &str, name: &str, id: &str, text: &str, ts: i64, own: bool| {
            let mut message = view("1203@g.us", "Team", id, text, ts, own);
            if !own {
                message.sender = Some(sender.to_string());
                message.sender_name = Some(name.to_string());
            }
            model.apply(&UiEvent::Message(message));
        };
        group(
            "1@s.whatsapp.net",
            "Ada",
            "d1",
            "morning! the deploy is green",
            now - 9 * 60_000,
            false,
        );
        group(
            "0@s.whatsapp.net",
            "You",
            "d2",
            "nice, shipping the docs now",
            now - 8 * 60_000,
            true,
        );
        group(
            "2@s.whatsapp.net",
            "Budi",
            "d3",
            "can someone review the PR?",
            now - 6 * 60_000,
            false,
        );
        group(
            "0@s.whatsapp.net",
            "You",
            "d4",
            "on it 👀",
            now - 5 * 60_000,
            true,
        );
        let mut document = view(
            "1203@g.us",
            "Team",
            "d5",
            "here's the report",
            now - 4 * 60_000,
            false,
        );
        document.kind = "document".to_string();
        document.sender = Some("1@s.whatsapp.net".to_string());
        document.sender_name = Some("Ada".to_string());
        document.document = Some(wa_types::DocumentView {
            name: "Q3-report.pdf".to_string(),
            mime: "application/pdf".to_string(),
            size: 1_842_000,
            pages: 12,
            thumb: None,
        });
        drop(group);
        model.apply(&UiEvent::Message(document));
        let mut note = view("1203@g.us", "Team", "d7", "", now - 2 * 60_000, false);
        note.kind = "audio".to_string();
        note.sender = Some("1@s.whatsapp.net".to_string());
        note.sender_name = Some("Ada".to_string());
        note.audio = Some(wa_types::AudioView {
            seconds: 12,
            ptt: true,
            waveform: (0..40).map(|index| ((index * 7) % 80 + 30) as u8).collect(),
        });
        model.apply(&UiEvent::Message(note));
        model.apply(&UiEvent::Message(view(
            "1203@g.us",
            "Team",
            "d6",
            "emoji check 👀 🙏 👍 🔥 😅 🎉",
            now - 3 * 60_000,
            false,
        )));

        model.apply(&UiEvent::Message(view(
            "1@s.whatsapp.net",
            "Ada",
            "a1",
            "hey, are we still on for tonight?",
            now - 3 * 60_000,
            false,
        )));
        model.apply(&UiEvent::Message(view(
            "1@s.whatsapp.net",
            "Ada",
            "a2",
            "i can push it to 8",
            now - 2 * 60_000,
            false,
        )));
        model.apply(&UiEvent::Message(view(
            "2@s.whatsapp.net",
            "Budi",
            "b1",
            "see you at 8",
            now - 90_000,
            true,
        )));
        model.active = Some("1203@g.us".to_string());
        model
    }

    pub fn apply(&mut self, event: &UiEvent) {
        match event {
            UiEvent::Qr { svg, .. } => self.login = fold(&self.login, &LoginEvent::Qr(svg.clone())),
            UiEvent::QrExpired => self.login = fold(&self.login, &LoginEvent::Expired),
            UiEvent::PairCode { code } => {
                self.login = fold(&self.login, &LoginEvent::PairCode(code.clone()))
            }
            UiEvent::Paired { name, .. } => {
                self.login = fold(&self.login, &LoginEvent::Paired);
                self.me_name = name.clone();
                self.status = format!("Linked as {name}");
            }
            UiEvent::Connected => {
                self.login = fold(&self.login, &LoginEvent::Connected);
                self.status = "Connected".to_string();
                self.connection_epoch += 1;
                // WhatsApp only pushes history to a newly linked device, so a
                // restart with an empty cache needs an explicit ask.
                if self.chats.is_empty() {
                    self.needs_history = true;
                }
            }
            UiEvent::Disconnected { reason } => {
                self.status = format!("Reconnecting… ({reason})");
            }
            UiEvent::LoggedOut { .. } => {
                self.login = fold(&self.login, &LoginEvent::LoggedOut);
                self.status = "Logged out".to_string();
                self.chats.clear();
                self.active = None;
            }
            UiEvent::PairError { message } => {
                self.status = message.clone();
            }
            UiEvent::Status(update) => self.add_status(update.clone()),
            UiEvent::StatusViewed {
                id,
                jid,
                name,
                timestamp,
            } => {
                for row in self.statuses.values_mut() {
                    if let Some(update) = row.updates.iter_mut().find(|u| &u.id == id) {
                        update.viewed = true;
                        if !update.viewers.iter().any(|viewer| &viewer.jid == jid) {
                            update.viewers.push(wa_ui::status::ViewerRow {
                                jid: jid.clone(),
                                name: name.clone(),
                                timestamp: *timestamp,
                            });
                        }
                    }
                }
            }
            UiEvent::ChannelInfo {
                jid,
                name,
                subscribers,
                ..
            } => {
                let row = self
                    .channels
                    .entry(jid.clone())
                    .or_insert_with(|| ChannelRow {
                        jid: jid.clone(),
                        ..Default::default()
                    });
                if !name.is_empty() {
                    row.name = name.clone();
                }
                row.subscribers = *subscribers;
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
            UiEvent::SelfIdentity { jid, name } => {
                if !jid.is_empty() {
                    self.me_jid = jid.clone();
                }
                if !name.is_empty() {
                    self.me_name = name.clone();
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
                if let Some(row) = self.chats.get_mut(chat) {
                    if *read {
                        row.unread = 0;
                    } else {
                        // The phone marked it unread: keep at least one badge.
                        row.unread = row.unread.max(1);
                    }
                }
            }
            UiEvent::Receipt { chat, ids, status } => {
                // A read (or played) receipt means the phone has read the chat:
                // the local badge must go, not just the bubble ticks.
                let read = matches!(status.as_str(), "read" | "played");
                let _ = ids;
                if let Some(row) = self.chats.get_mut(chat)
                    && read
                {
                    row.unread = 0;
                }
            }
            UiEvent::ChatAlias { from, to } => {
                // The same peer under a LID and its phone number: fold them.
                if let Some(row) = self.chats.remove(from) {
                    let target = self.chats.entry(to.clone()).or_insert_with(|| ChatRow {
                        jid: to.clone(),
                        ..Default::default()
                    });
                    if target.name.is_empty() {
                        target.name = row.name;
                    }
                    target.unread += row.unread;
                    target.last_ts = target.last_ts.max(row.last_ts);
                    for message in row.messages {
                        if !target.messages.iter().any(|known| known.id == message.id) {
                            target.messages.push(message);
                        }
                    }
                    target.messages.sort_by_key(|message| message.timestamp);
                }
                if self.active.as_deref() == Some(from.as_str()) {
                    self.active = Some(to.clone());
                }
            }
            UiEvent::ChatDeleted { chat } if self.active.as_deref() == Some(chat.as_str()) => {
                self.active = None;
            }
            UiEvent::ChatPin { chat, .. }
            | UiEvent::ChatArchive { chat, .. }
            | UiEvent::ChatMute { chat, .. } => {
                // App-state mentions chats we have never seen - pinned, archived
                // or muted groups are the usual case. Remember the JID so the
                // sidebar can list it (the name arrives from group metadata).
                self.chats.entry(chat.clone()).or_insert_with(|| ChatRow {
                    jid: chat.clone(),
                    ..Default::default()
                });
            }
            UiEvent::StarMessage { id, starred, .. } => {
                if *starred {
                    self.starred.insert(id.clone());
                } else {
                    self.starred.remove(id);
                }
            }
            UiEvent::MessageDeleted { chat, id } => {
                self.remove_message(chat, id);
            }
            UiEvent::MessageEdited { chat, id, text } => {
                if let Some(row) = self.chats.get_mut(chat)
                    && let Some(message) = row.messages.iter_mut().find(|m| &m.id == id)
                {
                    message.text = text.clone();
                }
            }
            UiEvent::Reaction {
                id, from_me, emoji, ..
            } => {
                self.apply_reaction(id, *from_me, emoji);
            }
            _ => {}
        }
    }

    /// Ask the session for a group's metadata (the window calls this when the
    /// group info screen opens).
    pub fn request_group(&mut self, chat: &str) -> bool {
        if chat.is_empty() || !chat.ends_with("@g.us") {
            return false;
        }
        self.group = None;
        self.group_error = None;
        true
    }

    pub fn group_loaded(&mut self, info: GroupInfoView) {
        if let Some(row) = self.chats.get_mut(&info.jid) {
            row.community = info.is_community;
            if !info.subject.is_empty() {
                row.name = info.subject.clone();
            }
        }
        self.group = Some(info);
    }

    pub fn group_failed(&mut self, reason: &str) {
        self.group = None;
        self.group_error = Some(reason.to_string());
    }

    /// The group subtitle (`Group - 10 members`), from `wa-ui`.
    pub fn group_count_label(&self) -> String {
        wa_ui::labels::group_member_count(self.group.as_ref().map(|g| g.members.len()).unwrap_or(0))
    }

    /// Members with the admin flag, admins first, then by name.
    pub fn group_members(&self) -> Vec<(String, bool, String)> {
        let Some(info) = self.group.as_ref() else {
            return Vec::new();
        };
        let mut rows: Vec<(String, bool, String)> = info
            .members
            .iter()
            .map(|member| {
                let name = self
                    .chats
                    .get(&member.jid)
                    .map(|row| row.name.clone())
                    .filter(|name| !name.is_empty())
                    .or_else(|| member.phone.clone())
                    .unwrap_or_else(|| {
                        member
                            .jid
                            .split('@')
                            .next()
                            .unwrap_or(&member.jid)
                            .to_string()
                    });
                (name, member.admin, member.jid.clone())
            })
            .collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        rows
    }

    /// Channels, newest activity first, with their follower line.
    pub fn channel_rows(&self) -> Vec<(ChannelRow, String)> {
        let mut rows: Vec<ChannelRow> = self.channels.values().cloned().collect();
        rows.sort_by_key(|row| {
            std::cmp::Reverse(self.chats.get(&row.jid).map(|c| c.last_ts).unwrap_or(0))
        });
        rows.into_iter()
            .map(|row| {
                let label = format_followers(row.subscribers as i64);
                (row, label)
            })
            .collect()
    }

    /// Posts of a channel, oldest first (the view scrolls to the newest).
    pub fn channel_posts(&self, jid: &str) -> Vec<MessageView> {
        let mut posts = self
            .chats
            .get(jid)
            .map(|row| row.messages.clone())
            .unwrap_or_default();
        posts.sort_by_key(|message| message.timestamp);
        posts
    }

    // ------------------------------------------------------------ in-chat search ---

    /// The indices of the open chat's messages that match the in-chat query.
    pub fn search_matches(&self) -> Vec<usize> {
        let Some(row) = self.active_row() else {
            return Vec::new();
        };
        let texts: Vec<String> = row
            .messages
            .iter()
            .map(|message| message.text.clone())
            .collect();
        wa_ui::search::matched_indices(&texts, &self.chat_search)
    }

    /// Restart the search from the first hit (the query changed).
    pub fn search_refresh(&mut self) {
        let total = self.search_matches().len();
        self.search_hit = wa_ui::search::keep_hit(total, 0);
    }

    /// Step to the next (or previous) hit; the counter wraps.
    pub fn search_step(&mut self, forward: bool) {
        let total = self.search_matches().len();
        if total == 0 {
            self.search_hit = -1;
            return;
        }
        let delta = if forward { 1 } else { -1 };
        self.search_hit = wa_ui::search::hit_index(total, self.search_hit.saturating_add(delta));
    }

    /// The `2/5` counter beside the search box.
    pub fn search_label(&self) -> String {
        let total = self.search_matches().len();
        wa_ui::search::count_label(self.search_hit, total, &self.chat_search).text
    }

    /// Which message the search bar points at, as an index into the chat.
    pub fn search_current(&self) -> Option<usize> {
        if self.search_hit < 0 {
            return None;
        }
        let matches = self.search_matches();
        let index = wa_ui::search::hit_index(matches.len(), self.search_hit);
        if index < 0 {
            return None;
        }
        matches.get(index as usize).copied()
    }

    // ------------------------------------------------------------------ emoji ---

    /// Emoji for the picker: recents first when the query is empty (the
    /// catalogue otherwise), or the query's matches.
    pub fn emoji_items(&self) -> Vec<String> {
        let query = self.emoji_query.trim();
        if query.is_empty() {
            let mut chars: Vec<String> = self.emoji_recents.clone();
            for group in wa_ui::emoji::groups() {
                for item in group.items {
                    if !chars.contains(&item.char) {
                        chars.push(item.char);
                    }
                    if chars.len() >= 160 {
                        return chars;
                    }
                }
            }
            return chars;
        }
        wa_ui::emoji::search(query, 160)
            .into_iter()
            .map(|item| item.char)
            .collect()
    }

    /// Put an emoji at the end of the composer and remember it.
    pub fn insert_emoji(&mut self, ch: &str) {
        let caret = self.composer.chars().count();
        let insertion = wa_ui::emoji::insert_text(&self.composer, caret, caret, ch);
        self.composer = insertion.value;
        self.emoji_recents = wa_ui::emoji::remember_recent(&self.emoji_recents, ch);
    }

    /// Merge a WhatsApp "Export chat" text file into one chat.
    pub fn import_export(&mut self, chat: &str, text: &str) -> (usize, usize) {
        use wa_ui::export::{ExistingMessage, import_plan, parse};

        let parsed = parse(text, false);
        if parsed.is_empty() {
            return (0, 0);
        }
        let existing: Vec<ExistingMessage> = self
            .chats
            .get(chat)
            .map(|row| {
                row.messages
                    .iter()
                    .map(|message| ExistingMessage {
                        id: message.id.clone(),
                        timestamp: message.timestamp,
                        text: message.text.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let plan = import_plan(&parsed, &existing);
        let removed = plan.remove_export_ids.len();
        let me = self.own_name();
        let chat_name = self
            .chats
            .get(chat)
            .map(|row| row.name.clone())
            .unwrap_or_default();
        let row = self
            .chats
            .entry(chat.to_string())
            .or_insert_with(|| ChatRow {
                jid: chat.to_string(),
                ..Default::default()
            });
        if row.name.is_empty() {
            row.name = chat_name;
        }
        row.messages
            .retain(|message| !plan.remove_export_ids.contains(&message.id));

        let mut added = 0;
        for message in plan.add {
            let from_me = message.sender.eq_ignore_ascii_case(&me)
                || message.sender.eq_ignore_ascii_case("you");
            row.messages.push(MessageView {
                chat: chat.to_string(),
                chat_name: row.name.clone(),
                sender: if from_me {
                    None
                } else {
                    Some(chat.to_string())
                },
                sender_name: if from_me {
                    None
                } else {
                    Some(message.sender.clone())
                },
                sender_phone: None,
                id: format!("export-{}-{added}", message.timestamp),
                from_me,
                is_group: chat.ends_with("@g.us"),
                timestamp: message.timestamp,
                kind: "text".to_string(),
                text: message.text,
                mentions: Vec::new(),
                reply_to: None,
                link: None,
                document: None,
                audio: None,
                stats: None,
                forwarded: false,
                once_recovered: false,
                status_quote: false,
            });
            added += 1;
        }
        row.messages.sort_by_key(|message| message.timestamp);
        row.last_ts = row
            .messages
            .last()
            .map(|message| message.timestamp)
            .unwrap_or(row.last_ts);
        (added, removed)
    }

    /// Star or unstar one message.
    pub fn toggle_star(&mut self, id: &str) -> bool {
        if self.starred.remove(id) {
            false
        } else {
            self.starred.insert(id.to_string());
            true
        }
    }

    // ------------------------------------------------------- message actions ---

    /// The menu rules for one message, straight from `wa-ui`.
    pub fn message_menu(&self, message: &MessageView) -> wa_ui::menus::MessageMenuOutput {
        wa_ui::menus::message_menu(&wa_ui::menus::MessageMenuInput {
            from_me: message.from_me,
            kind: message.kind.clone(),
            deleted: false,
            deleted_for_me: false,
            text: message.text.clone(),
            sender_label: message
                .sender_name
                .clone()
                .or_else(|| message.sender_phone.clone())
                .unwrap_or_else(|| "this contact".to_string()),
            chat_is_group: message.is_group,
            has_sender: message.sender.is_some(),
            starred: self.starred.contains(&message.id),
        })
    }

    /// Every starred message, newest first, with its chat JID.
    pub fn starred_messages(&self) -> Vec<(String, MessageView)> {
        let mut rows: Vec<(String, MessageView)> = self
            .chats
            .values()
            .flat_map(|row| {
                row.messages
                    .iter()
                    .filter(|message| self.starred.contains(&message.id))
                    .map(|message| (row.jid.clone(), message.clone()))
            })
            .collect();
        rows.sort_by_key(|(_, message)| std::cmp::Reverse(message.timestamp));
        rows
    }
    pub fn reply_preview(&self) -> Option<wa_ui::actions::ReplyPreview> {
        let source = self.reply_to.as_ref()?;
        Some(wa_ui::actions::reply_preview(
            &wa_ui::actions::ReplySource {
                from_me: source.from_me,
                kind: source.kind.clone(),
                text: source.text.clone(),
                sender: source.sender.clone().unwrap_or_default(),
                sender_name: source.sender_name.clone().unwrap_or_default(),
                contact_name: source.sender_name.clone().unwrap_or_default(),
            },
        ))
    }

    pub fn start_reply(&mut self, message: &MessageView) {
        self.reply_to = Some(message.clone());
        self.editing = None;
    }

    /// Edit mode loads the message into the composer.
    pub fn start_edit(&mut self, message: &MessageView) {
        self.editing = Some(message.clone());
        self.reply_to = None;
        self.composer = message.text.clone();
    }

    /// Leave reply/edit mode; an abandoned edit drops its draft.
    pub fn cancel_composer_mode(&mut self) {
        if self.editing.take().is_some() {
            self.composer.clear();
        }
        self.reply_to = None;
    }

    /// The send button's label while editing.
    pub fn send_label(&self) -> &'static str {
        if self.editing.is_some() {
            "Edit"
        } else {
            "Send"
        }
    }

    pub fn start_delete(&mut self, message: &MessageView) {
        self.delete_target = Some(message.clone());
    }

    /// The delete dialog's question and whether "for everyone" is offered.
    pub fn delete_question(&self) -> (String, bool) {
        let Some(message) = self.delete_target.as_ref() else {
            return (String::new(), false);
        };
        let output = wa_ui::menus::delete_confirm(
            &[wa_ui::menus::DeleteItem {
                from_me: message.from_me,
                deleted: false,
                chat: message.chat.clone(),
            }],
            message.chat == self.me_jid,
        );
        (output.title, output.can_everyone)
    }

    pub fn start_forward(&mut self, message: &MessageView) {
        self.forward_target = Some(message.clone());
        self.forward_search.clear();
        self.forward_chosen.clear();
    }

    /// Chats the forward picker lists, filtered by its search box.
    pub fn forward_rows(&self) -> Vec<&ChatRow> {
        let needle = self.forward_search.trim().to_lowercase();
        let mut rows: Vec<&ChatRow> = self
            .chats
            .values()
            .filter(|row| !row.jid.ends_with("@newsletter"))
            .filter(|row| needle.is_empty() || row.name.to_lowercase().contains(&needle))
            .collect();
        rows.sort_by(|a, b| {
            b.last_ts
                .cmp(&a.last_ts)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        rows
    }

    pub fn toggle_forward(&mut self, jid: &str) {
        if let Some(index) = self.forward_chosen.iter().position(|chosen| chosen == jid) {
            self.forward_chosen.remove(index);
        } else {
            self.forward_chosen.push(jid.to_string());
        }
    }

    /// The reaction pills under a message: (emoji, count, ours).
    pub fn reactions_for(&self, id: &str) -> Vec<(String, usize, bool)> {
        self.reactions
            .get(id)
            .map(|entry| {
                entry
                    .iter()
                    .map(|(emoji, (count, mine))| (emoji.clone(), *count, *mine))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Fold one reaction: ours replaces our previous emoji, and an empty one
    /// removes it, like `wa_ui::reactions` does for the WebView shell.
    fn apply_reaction(&mut self, id: &str, from_me: bool, emoji: &str) {
        let entry = self.reactions.entry(id.to_string()).or_default();
        let emoji = emoji.trim();
        if from_me {
            entry.retain(|_, (_, mine)| !*mine);
        }
        if emoji.is_empty() {
            if entry.is_empty() {
                self.reactions.remove(id);
            }
            return;
        }
        let slot = entry.entry(emoji.to_string()).or_insert((0, false));
        slot.0 += 1;
        if from_me {
            slot.1 = true;
        }
    }

    /// Show our own reaction straight away; the server echo replaces it.
    pub fn apply_own_reaction(&mut self, id: &str, emoji: &str) {
        self.apply_reaction(id, true, emoji);
    }

    /// Drop a message and its reactions (delete-for-me is local only).
    pub fn remove_message(&mut self, chat: &str, id: &str) {
        if let Some(row) = self.chats.get_mut(chat) {
            row.messages.retain(|message| message.id != id);
        }
        self.reactions.remove(id);
    }

    // ---------------------------------------------------------------- stickers ---

    /// Stickers already downloaded from the chats, newest first: (id, file).
    pub fn sticker_collection(&self) -> Vec<(String, String)> {
        let mut rows: Vec<(i64, String, String)> = self
            .chats
            .values()
            .flat_map(|row| row.messages.iter())
            .filter(|message| message.kind == "sticker")
            .filter_map(|message| match self.media.get(&message.id) {
                Some(MediaState::Path(path)) => {
                    Some((message.timestamp, message.id.clone(), path.clone()))
                }
                _ => None,
            })
            .collect();
        rows.sort_by_key(|row| std::cmp::Reverse(row.0));
        rows.into_iter().map(|(_, id, path)| (id, path)).collect()
    }

    pub fn favorites_loaded(&mut self, favorites: Vec<FavoriteStickerView>) {
        self.favorites = favorites;
        self.favorites_error = None;
    }

    pub fn favorites_failed(&mut self, reason: &str) {
        self.favorites = Vec::new();
        self.favorites_error = Some(reason.to_string());
    }

    // ----------------------------------------------------------- media viewer ---

    /// The open chat's photos and videos, in order: what the viewer pages.
    pub fn media_items(&self) -> Vec<MessageView> {
        let Some(row) = self.active_row() else {
            return Vec::new();
        };
        row.messages
            .iter()
            .filter(|message| matches!(message.kind.as_str(), "image" | "video"))
            .cloned()
            .collect()
    }

    /// Open the viewer on one message; false when it is not viewable media.
    pub fn open_viewer(&mut self, id: &str) -> bool {
        let Some(index) = self.media_items().iter().position(|item| item.id == id) else {
            return false;
        };
        self.media_viewer = Some(index);
        true
    }

    /// Step the viewer; past either end it closes, like Esc.
    pub fn step_viewer(&mut self, forward: bool) {
        let total = self.media_items().len();
        let Some(index) = self.media_viewer else {
            return;
        };
        if total == 0 {
            self.media_viewer = None;
            return;
        }
        let next = if forward {
            index as i64 + 1
        } else {
            index as i64 - 1
        };
        if next < 0 || next >= total as i64 {
            self.media_viewer = None;
            return;
        }
        self.media_viewer = Some(next as usize);
    }

    pub fn close_viewer(&mut self) {
        self.media_viewer = None;
    }

    /// The item the viewer is showing.
    pub fn viewer_item(&self) -> Option<MessageView> {
        let index = self.media_viewer?;
        self.media_items().get(index).cloned()
    }

    /// Where a message's media landed on disk, once it has downloaded.
    pub fn media_path(&self, id: &str) -> Option<String> {
        match self.media.get(id) {
            Some(MediaState::Path(path)) => Some(path.clone()),
            _ => None,
        }
    }

    /// Copy a message's cached media into `Downloads\WA Desk`.
    pub fn save_media(&self, id: &str) -> Result<String, String> {
        let path = self
            .media_path(id)
            .ok_or_else(|| "that media is not downloaded yet".to_string())?;
        let source = std::path::Path::new(&path);
        let name = source
            .file_name()
            .ok_or_else(|| "the cached file has no name".to_string())?;
        let dir = downloads_dir();
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let target = dir.join(name);
        std::fs::copy(source, &target).map_err(|err| err.to_string())?;
        Ok(target.to_string_lossy().into_owned())
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

    /// Chat rows for the sidebar, filtered by the search box and the pill.
    pub fn visible_chats(&self) -> Vec<&ChatRow> {
        let needle = self.search.trim().to_lowercase();
        let mut rows: Vec<&ChatRow> = self
            .chats
            .values()
            .filter(|row| !row.jid.ends_with("@newsletter"))
            .filter(|row| {
                !row.messages.is_empty() || !row.name.is_empty() || row.jid.ends_with("@g.us")
            })
            .filter(|row| match self.chat_filter {
                ChatFilter::All => true,
                ChatFilter::Groups => row.jid.ends_with("@g.us"),
                ChatFilter::Communities => row.community,
                ChatFilter::Unread => row.unread > 0,
            })
            .filter(|row| {
                needle.is_empty()
                    || row.name.to_lowercase().contains(&needle)
                    || row.jid.to_lowercase().contains(&needle)
            })
            .collect();
        rows.sort_by(|a, b| {
            b.last_ts
                .cmp(&a.last_ts)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        rows
    }

    /// Counts for the filter pills: (groups, communities, unread chats).
    pub fn filter_counts(&self) -> (usize, usize, usize) {
        let rows = self
            .chats
            .values()
            .filter(|row| !row.jid.ends_with("@newsletter"));
        let groups = rows
            .clone()
            .filter(|row| row.jid.ends_with("@g.us"))
            .count();
        let communities = rows.clone().filter(|row| row.community).count();
        let unread = rows.filter(|row| row.unread > 0).count();
        (groups, communities, unread)
    }

    /// Unread messages across every chat, for the rail badge.
    pub fn total_unread(&self) -> i64 {
        self.chats
            .values()
            .filter(|row| !row.jid.ends_with("@newsletter"))
            .map(|row| row.unread)
            .sum()
    }

    /// Apply the restart action's outcome (a fresh QR replaces the state).
    pub fn login_restarted(&mut self) {
        self.login = LoginState::Waiting;
        self.status = "Requesting a new code…".to_string();
    }

    fn add_status(&mut self, update: StatusUpdate) {
        let jid = update.sender.clone();
        let row = self
            .statuses
            .entry(jid.clone())
            .or_insert_with(|| StatusRow {
                jid,
                ..Default::default()
            });
        if !update.name.is_empty() {
            row.name = update.name.clone();
        }
        let view = StatusUpdateView {
            id: update.id,
            sender: update.sender,
            name: update.name,
            kind: update.kind,
            text: update.text,
            background: update.background.unwrap_or_default(),
            timestamp: update.timestamp,
            viewed: update.from_me,
            from_me: update.from_me,
            viewers: Vec::new(),
        };
        row.updates.retain(|existing| existing.id != view.id);
        row.updates.push(view);
        row.updates.sort_by_key(|u| u.timestamp);
    }

    fn status_request(&self) -> StatusRequest {
        StatusRequest {
            entries: self
                .statuses
                .values()
                .map(|row| StatusEntryView {
                    jid: row.jid.clone(),
                    name: row.name.clone(),
                    updates: row.updates.clone(),
                })
                .collect(),
            me: "Me".to_string(),
            me_jid: self.me_jid.clone(),
            me_lid: String::new(),
            now: self.now,
            photos: Default::default(),
            local_media: Default::default(),
        }
    }

    /// The panel's rows: newest story first, unseen flag from the frames.
    pub fn status_groups(&self) -> Vec<StatusGroupRow> {
        viewer_groups(&self.status_request())
            .into_iter()
            .map(|group| {
                let row = self.statuses.get(&group.jid);
                let unseen = row
                    .map(|row| row.updates.iter().any(|u| !u.viewed))
                    .unwrap_or(false);
                StatusGroupRow {
                    jid: group.jid.clone(),
                    name: group.name.clone(),
                    count: group.update_ids.len(),
                    unseen,
                }
            })
            .collect()
    }

    /// Open a poster's story at their first unseen frame.
    pub fn open_status(&mut self, jid: &str) {
        let groups = self.status_groups();
        let Some(index) = groups.iter().position(|group| group.jid == jid) else {
            return;
        };
        self.viewer = Some((index, 0));
        self.frame_started = Some(std::time::Instant::now());
        if let Some(row) = self.statuses.values_mut().find(|row| row.jid == jid) {
            for update in row.updates.iter_mut() {
                update.viewed = true;
            }
        }
    }

    pub fn close_status(&mut self) {
        self.viewer = None;
        self.frame_started = None;
    }

    /// The update the viewer is showing.
    pub fn current_status(&self) -> Option<&StatusUpdateView> {
        let (group, frame) = self.viewer?;
        let groups = self.status_groups();
        let row = self.statuses.get(&groups.get(group)?.jid)?;
        row.updates.get(frame)
    }

    /// Step the viewer through frames and groups with `wa-ui`'s rules.
    pub fn step_status(&mut self, forward: bool) {
        let Some((group, frame)) = self.viewer else {
            return;
        };
        let groups = self.status_groups();
        let frames = groups
            .get(group)
            .and_then(|row| self.statuses.get(&row.jid))
            .map(|row| row.updates.len())
            .unwrap_or(0);
        match next_position(group, frame, frames, groups.len(), forward) {
            Some(target) => {
                let frame = target.frame.unwrap_or_else(|| {
                    groups
                        .get(target.group)
                        .and_then(|row| self.statuses.get(&row.jid))
                        .map(|row| row.updates.len().saturating_sub(1))
                        .unwrap_or(0)
                });
                self.viewer = Some((target.group, frame));
                self.frame_started = Some(std::time::Instant::now());
            }
            None => self.close_status(),
        }
    }

    /// The active frame's fill (0..=1), and the chrome the viewer shows.
    pub fn status_progress(&self) -> f64 {
        let Some(update) = self.current_status() else {
            return 0.0;
        };
        let total = wa_ui::status::frame_duration(&update.kind, &update.text);
        let elapsed = self
            .frame_started
            .map(|started| started.elapsed().as_millis() as i64)
            .unwrap_or(0);
        progress_ratio(total, total - elapsed)
    }

    pub fn status_chrome(&self) -> Option<wa_ui::status::ViewerChrome> {
        let update = self.current_status()?;
        let (group, frame) = self.viewer?;
        let groups = self.status_groups();
        let frames = groups
            .get(group)
            .and_then(|row| self.statuses.get(&row.jid))
            .map(|row| row.updates.len())
            .unwrap_or(0);
        Some(viewer_chrome(&ViewerChromeInput {
            own: update.from_me,
            has_frame: true,
            paused: false,
            group_index: group,
            group_count: groups.len(),
            frame_index: frame,
            frame_count: frames,
            viewers: update.viewers.len(),
        }))
    }

    /// Ask for a message's media exactly once; the window calls this while
    /// painting a bubble whose bytes are not on disk yet.
    pub fn request_media(&mut self, id: &str) -> bool {
        if id.is_empty() || self.media.contains_key(id) {
            return false;
        }
        self.media.insert(id.to_string(), MediaState::Loading);
        true
    }

    pub fn media_ready(&mut self, id: &str, path: &str) {
        self.media
            .insert(id.to_string(), MediaState::Path(path.to_string()));
    }

    pub fn media_failed(&mut self, id: &str) {
        self.media.insert(id.to_string(), MediaState::Failed);
    }

    /// What a bubble shows when the media itself is not painted: the caption
    /// when there is one, otherwise the kind's word from `wa-ui`.
    pub fn bubble_body(&self, message: &MessageView) -> String {
        if !message.text.trim().is_empty() {
            return message.text.clone();
        }
        if matches!(
            message.kind.as_str(),
            "image" | "video" | "audio" | "document" | "sticker"
        ) {
            return wa_ui::chat::media_placeholder(&message.kind).to_string();
        }
        format!("[{}]", message.kind)
    }

    /// Open a chat: it becomes active and its badge clears. The in-chat search
    /// belongs to the chat it was started in, so it resets.
    pub fn open_chat(&mut self, jid: &str) {
        self.active = Some(jid.to_string());
        if let Some(row) = self.chats.get_mut(jid) {
            row.unread = 0;
        }
        self.search_open = false;
        self.chat_search.clear();
        self.search_hit = -1;
        self.media_viewer = None;
        self.reply_to = None;
        self.editing = None;
        self.delete_target = None;
        self.forward_target = None;
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

    /// The account's own display name, for the sidebar avatar.
    pub fn own_name(&self) -> String {
        if self.me_name.trim().is_empty() {
            "You".to_string()
        } else {
            self.me_name.clone()
        }
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

/// A chat as it is stored in the local cache.
#[derive(serde::Serialize, serde::Deserialize)]
struct CachedChat {
    jid: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    unread: i64,
    #[serde(default)]
    last_ts: i64,
    #[serde(default)]
    community: bool,
    #[serde(default)]
    messages: Vec<MessageView>,
}

impl Model {
    /// The chat list and recent messages, for the restart cache.
    pub fn cache_json(&self) -> String {
        let chats: Vec<CachedChat> = self
            .chats
            .values()
            .filter(|row| !row.jid.ends_with("@newsletter"))
            .map(|row| CachedChat {
                jid: row.jid.clone(),
                name: row.name.clone(),
                unread: row.unread,
                last_ts: row.last_ts,
                community: row.community,
                // Keep the file small: the newest 80 bubbles per chat.
                messages: row.messages.iter().rev().take(80).rev().cloned().collect(),
            })
            .collect();
        serde_json::json!({ "me": self.me_name, "chats": chats }).to_string()
    }

    pub fn load_cache(&mut self, json: &str) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
            return;
        };
        if let Some(me) = value["me"].as_str() {
            self.me_name = me.to_string();
        }
        let Some(chats) = value["chats"].as_array() else {
            return;
        };
        for chat in chats {
            let Ok(row) = serde_json::from_value::<CachedChat>(chat.clone()) else {
                continue;
            };
            // Ignore rows written by older builds that stored bare numbers.
            if !row.jid.contains('@') {
                continue;
            }
            self.chats.insert(
                row.jid.clone(),
                ChatRow {
                    jid: row.jid,
                    name: row.name,
                    messages: row.messages,
                    unread: row.unread,
                    last_ts: row.last_ts,
                    community: row.community,
                },
            );
        }
    }

    pub fn load_cache_file(&mut self, path: &std::path::Path) {
        if let Ok(text) = std::fs::read_to_string(path) {
            self.load_cache(&text);
        }
    }

    pub fn save_cache_file(&self, path: &std::path::Path) {
        let _ = std::fs::write(path, self.cache_json());
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

/// Where the media viewer's save button copies files, like WhatsApp Web's
/// `Downloads\WA Desk`.
fn downloads_dir() -> std::path::PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let dir = std::path::PathBuf::from(home).join("Downloads");
    if dir.is_dir() {
        dir.join("WA Desk")
    } else {
        std::path::PathBuf::from("WA Desk")
    }
}

/// Where profile photos are cached, so the next launch shows them instantly.
pub fn avatar_dir() -> std::path::PathBuf {
    let base = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("io.github.dirttw.wadesk-native")
        .join("avatars");
    let _ = std::fs::create_dir_all(&base);
    base
}

/// The cache file for one contact's photo.
pub fn avatar_path(jid: &str) -> std::path::PathBuf {
    let safe: String = jid
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    avatar_dir().join(format!("{safe}.img"))
}

/// Headless verification: feed the model the same events the phone would send
/// and check what the window would paint.
pub fn selftest() -> Result<String, String> {
    let mut model = Model::new();
    let now = 1_770_000_000_000;
    model.now = now;

    // The gate: QR -> its state, then pairing leaves it.
    model.apply(&UiEvent::Qr {
        svg: "<svg/>".into(),
        timeout_secs: 20,
    });
    if model.login != crate::login::LoginState::Qr("<svg/>".into()) {
        return Err(format!("qr should fill the gate, got {:?}", model.login));
    }
    if crate::login::can_restart(&model.login) {
        return Err("a live QR must not offer a restart".to_string());
    }
    model.apply(&UiEvent::QrExpired);
    if !crate::login::can_restart(&model.login) {
        return Err("an expired code must offer a restart".to_string());
    }
    model.login_restarted();
    model.apply(&UiEvent::PairCode {
        code: "ABCD-1234".into(),
    });
    if model.login != crate::login::LoginState::PairCode("ABCD-1234".into()) {
        return Err("pair code should show its own screen".to_string());
    }
    model.apply(&UiEvent::Connected);
    if !model.login.is_linked() {
        return Err("connecting must leave the gate".to_string());
    }
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

    // Media: a photo without bytes shows its kind's word, and the download
    // is requested exactly once.
    let mut photo = view("1@s.whatsapp.net", "Ada", "img1", "", now + 4_000, false);
    photo.kind = "image".to_string();
    model.apply(&UiEvent::Message(photo.clone()));
    if model.bubble_body(&photo) != "Photo" {
        return Err(format!(
            "a photo should label itself, got {:?}",
            model.bubble_body(&photo)
        ));
    }
    if !model.request_media(&photo.id) {
        return Err("the first request must start a download".to_string());
    }
    if model.request_media(&photo.id) {
        return Err("a second request must not duplicate the download".to_string());
    }
    model.media_ready(&photo.id, "C:/media/img1.jpg");
    if !matches!(model.media.get(&photo.id), Some(MediaState::Path(_))) {
        return Err("the cached path should land in the model".to_string());
    }

    // Status: two posters, newest first, unseen rings, the viewer steps.
    model.now = now;
    model.apply(&UiEvent::Status(StatusUpdate {
        id: "s-old".into(),
        sender: "9@s.whatsapp.net".into(),
        name: "Older".into(),
        kind: "text".into(),
        text: "old story".into(),
        background: Some("#243640".into()),
        timestamp: now - 60_000,
        from_me: false,
    }));
    model.apply(&UiEvent::Status(StatusUpdate {
        id: "s-new".into(),
        sender: "8@s.whatsapp.net".into(),
        name: "Newer".into(),
        kind: "text".into(),
        text: "new story".into(),
        background: None,
        timestamp: now - 1_000,
        from_me: false,
    }));
    // Channels: followed, listed with their follower line, posts read-only.
    model.apply(&UiEvent::ChannelInfo {
        jid: "1203@newsletter".into(),
        name: "BMKG".into(),
        subscribers: 84_300,
        picture: None,
    });
    let mut post = view(
        "1203@newsletter",
        "BMKG",
        "c1",
        "cuaca hari ini",
        now + 5_000,
        false,
    );
    post.chat_name = "BMKG".into();
    model.apply(&UiEvent::Message(post));
    let channels = model.channel_rows();
    if channels.len() != 1 || channels[0].0.name != "BMKG" {
        return Err(format!("one channel expected, got {channels:?}"));
    }
    if channels[0].1 != "84.3k followers" {
        return Err(format!("follower line wrong: {}", channels[0].1));
    }
    let posts = model.channel_posts("1203@newsletter");
    if posts.len() != 1 || posts[0].text != "cuaca hari ini" {
        return Err("the channel post should be listed".to_string());
    }
    // Channels never appear in the chat sidebar.
    if model
        .visible_chats()
        .iter()
        .any(|row| row.jid == "1203@newsletter")
    {
        return Err("a channel must not be listed as a chat".to_string());
    }
    let groups = model.status_groups();
    if groups.len() != 2 || groups[0].name != "Newer" {
        return Err(format!(
            "status groups should be newest first, got {groups:?}"
        ));
    }
    if !groups.iter().all(|group| group.unseen) {
        return Err("both posters start unseen".to_string());
    }
    model.open_status("8@s.whatsapp.net");
    let chrome = model
        .status_chrome()
        .ok_or("the viewer should have chrome")?;
    if chrome.own_story || !chrome.prev_hidden || chrome.next_hidden {
        return Err(format!(
            "first of two frames should offer next only, got {chrome:?}"
        ));
    }
    if model.status_progress() < 0.0 || model.status_progress() > 1.0 {
        return Err("progress must stay within 0..1".to_string());
    }
    if !model
        .current_status()
        .map(|u| u.id == "s-new")
        .unwrap_or(false)
    {
        return Err("the viewer should show the opened story".to_string());
    }
    // Watching it marks the poster seen in the list.
    if model
        .status_groups()
        .iter()
        .any(|g| g.name == "Newer" && g.unseen)
    {
        return Err("an opened story should not stay unseen".to_string());
    }
    model.step_status(true);
    if !model
        .current_status()
        .map(|u| u.id == "s-old")
        .unwrap_or(false)
    {
        return Err("stepping should move to the other poster".to_string());
    }
    model.close_status();
    if model.viewer.is_some() {
        return Err("closing clears the viewer".to_string());
    }
    // Settings: the theme resolves from the choice, the AI form validates
    // through wa-ui and a blank key keeps the stored one.
    let mut prefs = crate::settings::NativeSettings::default();
    if prefs.resolved_theme(true) != "dark" {
        return Err("system theme should follow a dark OS".to_string());
    }
    prefs.theme = "light".into();
    if prefs.resolved_theme(true) != "light" {
        return Err("an explicit theme must win".to_string());
    }
    let plan = prefs.ai_plan();
    if !plan.ok || !plan.settings["apiKey"].is_null() {
        return Err(format!(
            "default AI form should validate with no key, got {plan:?}"
        ));
    }
    prefs.ai.base_url = "ftp://bad".into();
    if prefs.ai_plan().ok {
        return Err("a non-http base URL must be rejected".to_string());
    }
    // Group info: the subtitle, admins first, admin flags kept.
    model.chats.insert(
        "6285@s.whatsapp.net".to_string(),
        ChatRow {
            jid: "6285@s.whatsapp.net".into(),
            name: "Budi".into(),
            ..Default::default()
        },
    );
    if !model.request_group("1203@g.us") {
        return Err("a group chat should ask for its metadata".to_string());
    }
    model.group_loaded(GroupInfoView {
        jid: "1203@g.us".into(),
        subject: "Team".into(),
        description: Some("shift swap".into()),
        members: vec![
            wa_types::GroupMemberView {
                jid: "6285@s.whatsapp.net".into(),
                phone: None,
                admin: false,
            },
            wa_types::GroupMemberView {
                jid: "6289@s.whatsapp.net".into(),
                phone: Some("+62 812-3456-789".into()),
                admin: true,
            },
        ],
        is_community: false,
        parent_jid: None,
        is_announce: false,
    });
    if model.group_count_label() != "Group - 2 members" {
        return Err(format!("subtitle wrong: {}", model.group_count_label()));
    }
    let members = model.group_members();
    if members[0].0 != "+62 812-3456-789" || !members[0].1 {
        return Err(format!("admins come first, got {members:?}"));
    }
    if members[1].0 != "Budi" {
        return Err("a known chat name should label the member".to_string());
    }

    // In-chat search: the open chat matches "again" once, and stepping wraps.
    model.open_chat("1@s.whatsapp.net");
    model.search_open = true;
    model.chat_search = "again".into();
    model.search_refresh();
    if model.search_label() != "1/1" || model.search_current().is_none() {
        return Err(format!(
            "search should hit once, got {:?}",
            model.search_label()
        ));
    }
    model.search_step(true);
    if model.search_current().is_none() {
        return Err("a single hit must keep pointing at itself".to_string());
    }

    // Emoji: an insert lands in the composer and leads the recents.
    model.composer.clear();
    model.insert_emoji("🙂");
    if model.composer != "🙂" || model.emoji_items().first().map(String::as_str) != Some("🙂") {
        return Err(format!("emoji insert wrong: {:?}", model.composer));
    }

    // Stickers: the downloaded one is offered, and sending is a command.
    if !model.sticker_collection().is_empty() {
        return Err("no sticker is downloaded yet".to_string());
    }
    model.media_ready("s1", "C:/media/s1.webp");

    // Media viewer: the photo opens, pages, and closes at the end.
    let items = model.media_items();
    if items.len() != 1 {
        return Err(format!(
            "one photo expected in the viewer, got {}",
            items.len()
        ));
    }
    if !model.open_viewer(&photo.id) {
        return Err("the photo should open in the viewer".to_string());
    }
    if model.viewer_item().map(|item| item.id) != Some(photo.id.clone()) {
        return Err("the viewer should show the opened photo".to_string());
    }
    model.step_viewer(true);
    if model.media_viewer.is_some() {
        return Err("stepping past the last photo closes the viewer".to_string());
    }

    // Message actions: reply banner, edit mode, delete dialog and the forward
    // picker all follow wa-ui's rules.
    model.apply(&UiEvent::Message(view(
        "1@s.whatsapp.net",
        "Ada",
        "mine1",
        "asli",
        now + 6_000,
        true,
    )));
    let incoming = view("1@s.whatsapp.net", "Ada", "in1", "halo", now + 7_000, false);
    model.apply(&UiEvent::Message(incoming.clone()));

    model.start_reply(&incoming);
    if model.reply_preview().is_none() {
        return Err("a pending reply should preview".to_string());
    }
    model.cancel_composer_mode();

    let mine = model.chats["1@s.whatsapp.net"]
        .messages
        .iter()
        .find(|message| message.id == "mine1")
        .cloned()
        .ok_or("the sent message should be stored")?;
    model.start_edit(&mine);
    if model.composer != "asli" || model.send_label() != "Edit" {
        return Err(format!(
            "edit mode wrong: {:?} / {}",
            model.composer,
            model.send_label()
        ));
    }
    model.cancel_composer_mode();

    model.start_delete(&mine);
    let (delete_title, delete_everyone) = model.delete_question();
    if delete_title.is_empty() || !delete_everyone {
        return Err(format!(
            "delete dialog wrong: {delete_title} / {delete_everyone}"
        ));
    }

    model.start_forward(&incoming);
    if model.forward_rows().is_empty() {
        return Err("the forward picker should offer chats".to_string());
    }
    model.toggle_forward("1@s.whatsapp.net");

    model.apply_own_reaction("in1", "👍");
    model.apply(&UiEvent::Reaction {
        chat: "1@s.whatsapp.net".into(),
        id: "in1".into(),
        sender: "1@s.whatsapp.net".into(),
        from_me: false,
        emoji: "❤️".into(),
        timestamp: now,
    });
    if model.reactions_for("in1").len() != 2 {
        return Err(format!(
            "two reaction pills expected, got {:?}",
            model.reactions_for("in1")
        ));
    }

    Ok(serde_json::json!({
        "chats": model.visible_chats().len(),
        "first": first,
        "preview": preview,
        "stamp": stamp,
        "status": model.status,
        "linked": model.login.is_linked(),
        "photo_body": model.bubble_body(&photo),
        "status_groups": model.status_groups().len(),
        "channels": model.channel_rows().len(),
        "theme": prefs.resolved_theme(true),
        "group_members": model.group_members().len(),
        "search": model.search_label(),
        "emoji": model.emoji_items().len(),
        "viewer_items": model.media_items().len(),
        "stickers": model.sticker_collection().len(),
        "reply": model.reply_preview().is_some(),
        "delete": delete_title,
        "delete_everyone": delete_everyone,
        "forward": model.forward_chosen.len(),
        "reactions": model.reactions_for("in1").len(),
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
    fn photos_are_requested_once_and_labelled() {
        let mut model = Model::new();
        let mut photo = view("1@s.whatsapp.net", "Ada", "img", "", 1, false);
        photo.kind = "image".to_string();
        assert_eq!(model.bubble_body(&photo), "Photo");
        assert!(model.request_media("img"));
        assert!(!model.request_media("img"), "one download per message");
        model.media_ready("img", "/tmp/x.jpg");
        assert_eq!(
            model.media.get("img"),
            Some(&MediaState::Path("/tmp/x.jpg".into()))
        );
        model.media_failed("img");
        assert_eq!(model.media.get("img"), Some(&MediaState::Failed));
        let mut clip = photo.clone();
        clip.kind = "video".to_string();
        assert_eq!(model.bubble_body(&clip), "Video");
        let mut doc = photo.clone();
        doc.kind = "document".to_string();
        doc.text = "report.pdf".to_string();
        assert_eq!(model.bubble_body(&doc), "report.pdf", "a caption wins");
    }

    #[test]
    fn reading_on_the_phone_clears_the_badge() {
        let mut model = Model::new();
        model.now = 1_770_000_000_000;
        // Two unread messages from Ada.
        model.apply(&UiEvent::Message(view(
            "1@s.whatsapp.net",
            "Ada",
            "m1",
            "hi",
            1,
            false,
        )));
        model.apply(&UiEvent::Message(view(
            "1@s.whatsapp.net",
            "Ada",
            "m2",
            "there",
            2,
            false,
        )));
        assert_eq!(model.chats["1@s.whatsapp.net"].unread, 2);

        // The phone reports them read: the badge disappears.
        model.apply(&UiEvent::Receipt {
            chat: "1@s.whatsapp.net".into(),
            ids: vec!["m1".into(), "m2".into()],
            status: "read".into(),
        });
        assert_eq!(model.chats["1@s.whatsapp.net"].unread, 0, "badge cleared");
        assert_eq!(model.chats["1@s.whatsapp.net"].unread, 0, "badge cleared");

        // A delivery receipt must not clear it again.
        model.apply(&UiEvent::Message(view(
            "1@s.whatsapp.net",
            "Ada",
            "m3",
            "yo",
            3,
            false,
        )));
        assert_eq!(model.chats["1@s.whatsapp.net"].unread, 1);
        model.apply(&UiEvent::Receipt {
            chat: "1@s.whatsapp.net".into(),
            ids: vec!["m1".into()],
            status: "delivered".into(),
        });
        assert_eq!(
            model.chats["1@s.whatsapp.net"].unread, 1,
            "delivered keeps it"
        );
    }

    #[test]
    fn search_finds_and_steps_through_hits() {
        let mut model = Model::new();
        model.active = Some("1@s.whatsapp.net".into());
        for (id, text, ts) in [
            ("m1", "hello there", 1),
            ("m2", "nothing here", 2),
            ("m3", "say HELLO again", 3),
        ] {
            model.apply(&UiEvent::Message(view(
                "1@s.whatsapp.net",
                "Ada",
                id,
                text,
                ts,
                false,
            )));
        }
        model.chat_search = "hello".into();
        model.search_refresh();
        assert_eq!(model.search_matches(), vec![0, 2]);
        assert_eq!(model.search_label(), "1/2");
        assert_eq!(model.search_current(), Some(0));
        model.search_step(true);
        assert_eq!(model.search_current(), Some(2));
        assert_eq!(model.search_label(), "2/2");
        model.search_step(true);
        assert_eq!(model.search_current(), Some(0), "the counter wraps");
        model.search_step(false);
        assert_eq!(model.search_current(), Some(2), "and wraps backwards");

        // A query with no hits disables the counter, like WhatsApp.
        model.chat_search = "zzz".into();
        model.search_refresh();
        assert_eq!(model.search_label(), "0/0");
        assert_eq!(model.search_current(), None);
    }

    #[test]
    fn emoji_land_in_the_composer_and_are_remembered() {
        let mut model = Model::new();
        model.composer = "hi ".into();
        assert!(model.emoji_items().len() >= 100, "the catalogue is offered");
        model.insert_emoji("🙂");
        model.insert_emoji("👍");
        assert_eq!(model.composer, "hi 🙂👍");
        assert_eq!(
            model.emoji_recents,
            vec!["👍", "🙂"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
        // Recents lead the empty-query grid.
        assert_eq!(model.emoji_items()[0], "👍");

        // A query searches the catalogue.
        model.emoji_query = "grin".into();
        let matches = model.emoji_items();
        assert!(!matches.is_empty(), "searching 'grin' finds faces");
        assert!(matches.len() <= 160);
    }

    #[test]
    fn the_viewer_pages_through_photos() {
        let mut model = Model::new();
        model.active = Some("1@s.whatsapp.net".into());
        let mut photo = view("1@s.whatsapp.net", "Ada", "p1", "", 1, false);
        photo.kind = "image".into();
        model.apply(&UiEvent::Message(photo));
        let mut clip = view("1@s.whatsapp.net", "Ada", "v1", "clip", 2, false);
        clip.kind = "video".into();
        model.apply(&UiEvent::Message(clip));
        model.apply(&UiEvent::Message(view(
            "1@s.whatsapp.net",
            "Ada",
            "t1",
            "not media",
            3,
            false,
        )));

        assert_eq!(model.media_items().len(), 2, "text is skipped");
        assert!(model.open_viewer("v1"), "a video opens too");
        assert_eq!(model.viewer_item().map(|item| item.id), Some("v1".into()));
        model.step_viewer(true);
        assert!(model.media_viewer.is_none(), "past the end closes");
        assert!(!model.open_viewer("t1"), "text is not viewable");

        // Saving needs the bytes; without them it says so.
        assert!(model.save_media("p1").is_err());
        model.media_ready("p1", "C:/media/p1.jpg");
        assert!(model.media_path("p1").is_some());
    }

    #[test]
    fn the_sticker_collection_lists_downloaded_stickers() {
        let mut model = Model::new();
        model.active = Some("1@s.whatsapp.net".into());
        let mut sticker = view("1@s.whatsapp.net", "Ada", "s1", "", 5, false);
        sticker.kind = "sticker".into();
        model.apply(&UiEvent::Message(sticker));
        assert!(model.sticker_collection().is_empty(), "not downloaded yet");
        model.media_ready("s1", "C:/media/s1.webp");
        let collection = model.sticker_collection();
        assert_eq!(collection.len(), 1);
        assert_eq!(collection[0].0, "s1");
        assert_eq!(collection[0].1, "C:/media/s1.webp");

        // Favourites come from the session and can fail loud.
        model.favorites_loaded(vec![wa_types::FavoriteStickerView {
            id: "fav-1".into(),
            path: "C:/media/fav-1.webp".into(),
            width: 512,
            height: 512,
        }]);
        assert_eq!(model.favorites.len(), 1);
        model.favorites_failed("no app-state yet");
        assert!(model.favorites.is_empty());
        assert_eq!(model.favorites_error.as_deref(), Some("no app-state yet"));
    }

    #[test]
    fn reply_and_edit_drive_the_composer() {
        let mut model = Model::new();
        let mut mine = view("1@s.whatsapp.net", "Ada", "m1", "hello", 1, true);
        mine.from_me = true;
        let mut theirs = view("2@g.us", "Team", "m2", "yo", 2, false);
        theirs.is_group = true;

        // Reply: the banner names the sender and previews the body.
        model.start_reply(&theirs);
        let preview = model.reply_preview().expect("a pending reply");
        assert!(!preview.preview.is_empty());
        assert_eq!(model.send_label(), "Send");
        model.cancel_composer_mode();
        assert!(model.reply_preview().is_none(), "cancel drops the reply");

        // Edit: our own text loads into the composer and the button changes.
        model.start_edit(&mine);
        assert_eq!(model.composer, "hello");
        assert_eq!(model.send_label(), "Edit");
        // Editing is only offered for our own text, per wa-ui.
        assert!(model.message_menu(&mine).edit);
        assert!(!model.message_menu(&theirs).edit);
        model.cancel_composer_mode();
        assert!(
            model.composer.is_empty(),
            "an abandoned edit drops its draft"
        );
        assert_eq!(model.send_label(), "Send");
    }

    #[test]
    fn the_delete_dialog_follows_message_ownership() {
        let mut model = Model::new();
        model.me_jid = "9@s.whatsapp.net".into();
        let mut mine = view("1@s.whatsapp.net", "Ada", "m1", "hello", 1, true);
        mine.from_me = true;
        let theirs = view("1@s.whatsapp.net", "Ada", "m2", "yo", 2, false);
        model.apply(&UiEvent::Message(mine.clone()));
        model.apply(&UiEvent::Message(theirs.clone()));

        model.start_delete(&mine);
        let (title, everyone) = model.delete_question();
        assert_eq!(title, "Delete message?");
        assert!(everyone, "our own message can go for everyone");

        model.start_delete(&theirs);
        let (_, everyone) = model.delete_question();
        assert!(!everyone, "someone else's message is only ours to hide");

        // Deleted messages leave the list and take their reactions with them.
        model.apply_own_reaction("m2", "👍");
        model.remove_message("1@s.whatsapp.net", "m2");
        assert!(
            model.chats["1@s.whatsapp.net"]
                .messages
                .iter()
                .all(|message| message.id != "m2"),
            "the deleted message is gone, the other stays"
        );
        assert_eq!(model.chats["1@s.whatsapp.net"].messages.len(), 1);
        assert!(model.reactions_for("m2").is_empty());
    }

    #[test]
    fn forwards_pick_chats_and_edits_revoke_live() {
        let mut model = Model::new();
        for (jid, name, ts) in [
            ("1@s.whatsapp.net", "Ada", 10),
            ("2@s.whatsapp.net", "Budi", 20),
        ] {
            model.apply(&UiEvent::Message(view(
                jid,
                name,
                &format!("{jid}-m"),
                "hi",
                ts,
                false,
            )));
        }
        model.start_forward(&model.chats["1@s.whatsapp.net"].messages[0].clone());
        assert_eq!(model.forward_rows().len(), 2, "every chat is offered");
        model.forward_search = "bud".into();
        assert_eq!(model.forward_rows().len(), 1, "search filters the picker");
        model.forward_search.clear();
        model.toggle_forward("1@s.whatsapp.net");
        model.toggle_forward("2@s.whatsapp.net");
        assert_eq!(model.forward_chosen.len(), 2, "multi-select");
        model.toggle_forward("1@s.whatsapp.net");
        assert_eq!(model.forward_chosen, vec!["2@s.whatsapp.net".to_string()]);

        // An edit from another device rewrites the stored body.
        model.apply(&UiEvent::MessageEdited {
            chat: "1@s.whatsapp.net".into(),
            id: "1@s.whatsapp.net-m".into(),
            text: "edited".into(),
        });
        assert_eq!(model.chats["1@s.whatsapp.net"].messages[0].text, "edited");

        // A revoke drops it.
        model.apply(&UiEvent::MessageDeleted {
            chat: "2@s.whatsapp.net".into(),
            id: "2@s.whatsapp.net-m".into(),
        });
        assert!(model.chats["2@s.whatsapp.net"].messages.is_empty());
    }

    #[test]
    fn reactions_replace_our_own_pill() {
        let mut model = Model::new();
        let mut message = view("1@s.whatsapp.net", "Ada", "m1", "hi", 1, false);
        message.from_me = false;
        model.apply(&UiEvent::Message(message));

        // Someone else reacts.
        model.apply(&UiEvent::Reaction {
            chat: "1@s.whatsapp.net".into(),
            id: "m1".into(),
            sender: "1@s.whatsapp.net".into(),
            from_me: false,
            emoji: "👍".into(),
            timestamp: 10,
        });
        assert_eq!(model.reactions_for("m1"), vec![("👍".into(), 1, false)]);

        // We react, then change our mind: ours replaces, it does not stack.
        model.apply_own_reaction("m1", "❤️");
        model.apply_own_reaction("m1", "😂");
        let pills = model.reactions_for("m1");
        assert_eq!(
            pills,
            vec![("👍".into(), 1, false), ("😂".into(), 1, true)],
            "one pill per emoji, ours flagged"
        );

        // An empty emoji from us removes our reaction only.
        model.apply(&UiEvent::Reaction {
            chat: "1@s.whatsapp.net".into(),
            id: "m1".into(),
            sender: "9@s.whatsapp.net".into(),
            from_me: true,
            emoji: String::new(),
            timestamp: 11,
        });
        assert_eq!(model.reactions_for("m1"), vec![("👍".into(), 1, false)]);
    }

    #[test]
    fn file_sizes_read_like_whatsapp() {
        assert_eq!(Model::file_size(812), "812 B");
        assert_eq!(Model::file_size(4_200), "4.2 kB");
        assert_eq!(Model::file_size(1_842_000), "1.8 MB");
        assert_eq!(Model::file_size(2_500_000_000), "2.5 GB");
    }

    #[test]
    fn the_chat_cache_round_trips() {
        let mut model = Model::new();
        model.now = 1_770_000_000_000;
        model.me_name = "You".into();
        model.apply(&UiEvent::Message(view(
            "1@s.whatsapp.net",
            "Ada",
            "m1",
            "hi",
            5,
            false,
        )));
        model.chats.get_mut("1@s.whatsapp.net").unwrap().unread = 2;

        let json = model.cache_json();
        let mut restored = Model::new();
        restored.load_cache(&json);
        assert_eq!(restored.me_name, "You");
        let row = &restored.chats["1@s.whatsapp.net"];
        assert_eq!(row.name, "Ada");
        assert_eq!(row.unread, 2);
        assert_eq!(row.messages.len(), 1);
        assert_eq!(row.messages[0].text, "hi");
        assert_eq!(restored.visible_chats().len(), 1);
    }

    #[test]
    fn contacts_seed_the_chat_list_before_history_arrives() {
        let mut model = Model::new();
        model.apply(&UiEvent::Chat {
            jid: "6281@s.whatsapp.net".into(),
            name: "Pak Yoga".into(),
            is_group: false,
            timestamp: 0,
        });
        let rows = model.visible_chats();
        assert_eq!(rows.len(), 1);
        assert_eq!(model.title(rows[0]), "Pak Yoga");
    }

    #[test]
    fn starring_lists_messages_across_chats() {
        let mut model = Model::new();
        model.apply(&UiEvent::Message(view(
            "1@s.whatsapp.net",
            "Ada",
            "m1",
            "hi",
            10,
            false,
        )));
        model.apply(&UiEvent::StarMessage {
            chat: "1@s.whatsapp.net".into(),
            id: "m1".into(),
            starred: true,
            timestamp: 10,
        });
        let rows = model.starred_messages();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1.id, "m1");
        assert!(model.message_menu(&rows[0].1).star_label.contains("Unstar"));

        model.apply(&UiEvent::StarMessage {
            chat: "1@s.whatsapp.net".into(),
            id: "m1".into(),
            starred: false,
            timestamp: 11,
        });
        assert!(model.starred_messages().is_empty());
    }

    #[test]
    fn the_selftest_passes() {
        let report = selftest().expect("selftest");
        assert!(report.contains("\"group_members\":2"));
    }
}
