//! The native front end's state: chats, messages and what the window shows.
//!
//! Everything user-visible that is *not* painting comes from `wa-ui`, so the
//! native window and the WebView cannot disagree about a timestamp, a preview or
//! a delivery tick.

use std::collections::{BTreeMap, HashMap};

use wa_types::{MessageView, StatusUpdate, UiEvent};
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
    /// Restart the pairing session for a fresh QR.
    Restart,
    /// Download (or find) one message's media.
    FetchMedia {
        id: String,
    },
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
}

#[derive(Debug, Default)]
pub struct Model {
    pub chats: BTreeMap<String, ChatRow>,
    pub active: Option<String>,
    /// The connection line under the title.
    pub status: String,
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
            UiEvent::Qr { svg, .. } => self.login = fold(&self.login, &LoginEvent::Qr(svg.clone())),
            UiEvent::QrExpired => self.login = fold(&self.login, &LoginEvent::Expired),
            UiEvent::PairCode { code } => {
                self.login = fold(&self.login, &LoginEvent::PairCode(code.clone()))
            }
            UiEvent::Paired { name, .. } => {
                self.login = fold(&self.login, &LoginEvent::Paired);
                self.status = format!("Linked as {name}");
            }
            UiEvent::Connected => {
                self.login = fold(&self.login, &LoginEvent::Connected);
                self.status = "Connected".to_string();
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
            .filter(|row| !row.jid.ends_with("@newsletter"))
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
    fn the_selftest_passes() {
        let report = selftest().expect("selftest");
        assert!(report.contains("\"chats\":2"));
    }
}
