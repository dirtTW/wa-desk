//! The window: a chat list on the left, the open conversation on the right.
//!
//! Painting is all this module does. Every string it shows comes from
//! `crate::model`, which in turn asks `wa-ui`.

use std::sync::mpsc::Sender;

use tokio::sync::mpsc::UnboundedReceiver;
use wa_types::{MessageView, UiEvent};

use crate::login::LoginState;
use crate::model::{BubbleAction, ChatRow, Command, Model};
use crate::settings::NativeSettings;
use crate::updates::{self, Release, UpdateMsg, UpdateState};

/// What `main` assembles before the window exists.
pub struct NativeApp {
    pub events: UnboundedReceiver<UiEvent>,
    pub commands: Sender<Command>,
    pub runtime: tokio::runtime::Handle,
    /// Results of media downloads, (message id, cached path or error).
    pub media: std::sync::mpsc::Receiver<(String, Result<String, String>)>,
    /// Group metadata replies, (chat, info or error).
    pub group: std::sync::mpsc::Receiver<(String, Result<wa_types::GroupInfoView, String>)>,
    /// Favourite-sticker replies.
    pub favorites: std::sync::mpsc::Receiver<Result<Vec<wa_types::FavoriteStickerView>, String>>,
    /// Profile photo URLs, (jid, url).
    pub avatars: std::sync::mpsc::Receiver<(String, Option<String>)>,
    /// Attachment send results (a note or an error).
    pub attach: std::sync::mpsc::Receiver<Result<String, String>>,
    /// The address book for the new-chat picker.
    pub contacts: std::sync::mpsc::Receiver<Vec<(String, String)>>,
}

impl NativeApp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        events: UnboundedReceiver<UiEvent>,
        commands: Sender<Command>,
        runtime: tokio::runtime::Handle,
        media: std::sync::mpsc::Receiver<(String, Result<String, String>)>,
        group: std::sync::mpsc::Receiver<(String, Result<wa_types::GroupInfoView, String>)>,
        favorites: std::sync::mpsc::Receiver<Result<Vec<wa_types::FavoriteStickerView>, String>>,
        avatars: std::sync::mpsc::Receiver<(String, Option<String>)>,
        attach: std::sync::mpsc::Receiver<Result<String, String>>,
        contacts: std::sync::mpsc::Receiver<Vec<(String, String)>>,
    ) -> Self {
        Self {
            events,
            commands,
            runtime,
            media,
            group,
            favorites,
            avatars,
            attach,
            contacts,
        }
    }
}

pub struct WaDesk {
    model: Model,
    events: UnboundedReceiver<UiEvent>,
    commands: Sender<Command>,
    runtime: tokio::runtime::Handle,
    typing_sent: bool,
    /// The status panel is showing instead of a conversation.
    show_status: bool,
    /// The settings screen is showing instead of a conversation.
    show_settings: bool,
    settings: NativeSettings,
    settings_path: std::path::PathBuf,
    /// The last save result, shown under the form.
    settings_note: String,
    /// The channels panel is showing instead of a conversation.
    show_channels: bool,
    /// Rasterised QR code, keyed by the SVG it came from so a rotating code
    /// is decoded once per rotation.
    qr: Option<(String, egui::TextureHandle)>,
    /// Media download results, drained every frame.
    media_rx: std::sync::mpsc::Receiver<(String, Result<String, String>)>,
    /// Group metadata replies.
    group_rx: std::sync::mpsc::Receiver<(String, Result<wa_types::GroupInfoView, String>)>,
    /// The group info screen is open.
    show_group: bool,
    /// Decoded media textures, keyed by message id.
    textures: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    /// Decoded sticker textures, keyed by sticker id or favourite id.
    sticker_textures: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    /// Favourite-sticker replies.
    favorites_rx: std::sync::mpsc::Receiver<Result<Vec<wa_types::FavoriteStickerView>, String>>,
    /// The search hit the list was last scrolled to.
    scrolled_hit: i64,
    /// A short status line for the viewer (saved path, failures).
    note: String,
    /// A transient toast ("Use the contact screen to call"), a few seconds.
    notice: Option<(String, std::time::Instant)>,
    /// Self-update state and its worker replies.
    update: UpdateState,
    update_tx: std::sync::mpsc::Sender<UpdateMsg>,
    update_rx: std::sync::mpsc::Receiver<UpdateMsg>,
    /// The automatic check has already run once.
    update_checked: bool,
    /// The resolved WhatsApp palette for this frame.
    palette: crate::theme::Palette,
    /// The OS dark-mode preference, captured before we install our own visuals.
    system_dark: bool,
    /// `--screenshot <path>`: save one frame and quit (used to verify the UI).
    screenshot: Option<(std::path::PathBuf, std::time::Instant, bool)>,
    /// The restart cache for the chat list (`chats.json` beside the session).
    cache_path: std::path::PathBuf,
    cache_dirty: bool,
    cache_saved: std::time::Instant,
    /// Profile photos: jid -> circular texture (None once known to be missing).
    avatars: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    /// Photos already asked for.
    avatar_asked: std::collections::HashSet<String>,
    /// JIDs whose photo was read from the disk cache.
    avatar_disk: std::collections::HashSet<String>,
    /// `WA_DESK_OPEN_FIRST`: opened the first chat once (screenshot testing).
    auto_opened: bool,
    avatar_rx: std::sync::mpsc::Receiver<(String, Option<String>)>,
    avatar_img_tx: std::sync::mpsc::Sender<(String, Result<Vec<u8>, String>)>,
    avatar_img_rx: std::sync::mpsc::Receiver<(String, Result<Vec<u8>, String>)>,
    /// Attachment results, and the short note under the composer.
    attach_rx: std::sync::mpsc::Receiver<Result<String, String>>,
    attach_note: Option<(String, std::time::Instant)>,
    /// True in `--preview`: never write the demo chats to the real cache.
    preview: bool,
    /// Screenshot support: start the demo video as soon as it is decoded.
    preview_autoplay: bool,
    /// Progressive history sync: batches left and when the last one went out.
    sync_rounds: u8,
    last_sync: std::time::Instant,
    /// The "new chat" FAB asked the search box to take focus.
    focus_search: bool,
    /// The connection epoch the backfill already started for.
    seen_epoch: u64,
    /// When the chat app-state was last polled for read marks/mutes.
    last_state_sync: std::time::Instant,
    /// When groups/status were last refreshed in full.
    last_full_sync: std::time::Instant,
    /// Prepared videos and their frame textures.
    videos: std::collections::HashMap<String, VideoState>,
    video_tx: std::sync::mpsc::Sender<(String, Result<crate::video::Video, String>)>,
    video_rx: std::sync::mpsc::Receiver<(String, Result<crate::video::Video, String>)>,
    video_textures: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    /// Insertion order of `video_textures`, so old frames can be evicted.
    video_order: std::collections::VecDeque<String>,
    video_playing: bool,
    video_started: Option<std::time::Instant>,
    video_offset: f32,
    video_muted: bool,
    /// Group JIDs whose metadata has been requested.
    groups_asked: std::collections::HashSet<String>,
    /// The Chats kebab menu is open.
    show_chat_menu: bool,
    /// Which Settings section is open.
    settings_section: usize,
    /// The audio player, kept alive for the window's lifetime.
    audio: crate::audio::Audio,
    /// The sticker picker tab (0 recents, 1 favourites, 2 packs) and search.
    sticker_tab: usize,
    sticker_query: String,
    /// The emoji picker's selected category (0 = recents, 1.. = catalogue groups).
    emoji_group: usize,
    /// The picker's GIF tab (no GIF search yet, so it explains itself).
    emoji_gif: bool,
    /// Colour emoji textures, keyed by (glyph, pixel size).
    emoji_textures: std::collections::HashMap<(char, u32), Option<egui::TextureHandle>>,
    /// The remote call video decoder, started on the first frame.
    call_video: Option<crate::callvideo::CallVideo>,
    /// The newest decoded remote frame, uploaded as a texture.
    call_frame: Option<egui::TextureHandle>,
    /// Preview hook: an image used as the self-view in `--preview`.
    call_self_source: Option<String>,
    /// Our own camera preview shown in the call sheet corner.
    call_self_frame: Option<egui::TextureHandle>,
    /// Which decoder generation call_frame holds.
    call_frame_generation: u64,
    /// The Windows tray icon and menu (None when the shell refuses it).
    tray: Option<crate::tray::Tray>,
    /// The open chat's actions menu.
    show_chat_actions: bool,
    /// The new-chat picker and its search.
    show_new_chat: bool,
    new_chat_search: String,
    contacts: Vec<(String, String)>,
    contacts_rx: std::sync::mpsc::Receiver<Vec<(String, String)>>,
    show_starred: bool,
    show_new_group: bool,
    new_group_subject: String,
    new_group_members: std::collections::HashSet<String>,
    show_new_contact: bool,
    new_contact_phone: String,
    new_contact_name: String,
    group_add_open: bool,
    group_add_members: std::collections::HashSet<String>,
}

impl WaDesk {
    pub fn new(_cc: &eframe::CreationContext<'_>, native: NativeApp) -> Self {
        install_fonts(&_cc.egui_ctx);
        let (update_tx, update_rx) = std::sync::mpsc::channel();
        let system_dark = _cc.egui_ctx.style().visuals.dark_mode;
        let settings = NativeSettings::load(&settings_file());
        let palette = crate::theme::palette(&settings.theme, system_dark);
        let cache_path =
            std::path::Path::new(&crate::model::store_path()).with_file_name("chats.json");
        let (avatar_img_tx, avatar_img_rx) = std::sync::mpsc::channel();
        let (video_tx, video_rx) = std::sync::mpsc::channel();
        let mut model = Model::new();
        model.load_cache_file(&cache_path);
        Self {
            model,
            events: native.events,
            commands: native.commands,
            runtime: native.runtime,
            typing_sent: false,
            show_status: false,
            show_channels: false,
            show_settings: false,
            settings,
            settings_path: settings_file(),
            settings_note: String::new(),
            qr: None,
            media_rx: native.media,
            group_rx: native.group,
            show_group: false,
            textures: std::collections::HashMap::new(),
            sticker_textures: std::collections::HashMap::new(),
            favorites_rx: native.favorites,
            scrolled_hit: -1,
            note: String::new(),
            notice: None,
            update: UpdateState::Idle,
            update_tx: update_tx.clone(),
            update_rx,
            update_checked: false,
            palette,
            system_dark,
            screenshot: None,
            cache_path,
            cache_dirty: false,
            cache_saved: std::time::Instant::now(),
            avatars: std::collections::HashMap::new(),
            avatar_asked: std::collections::HashSet::new(),
            avatar_disk: std::collections::HashSet::new(),
            auto_opened: false,
            avatar_rx: native.avatars,
            avatar_img_tx,
            avatar_img_rx,
            attach_rx: native.attach,
            attach_note: None,
            preview: false,
            preview_autoplay: false,
            sync_rounds: 6,
            last_sync: std::time::Instant::now(),
            focus_search: false,
            seen_epoch: 0,
            last_state_sync: std::time::Instant::now(),
            last_full_sync: std::time::Instant::now(),
            videos: std::collections::HashMap::new(),
            video_tx: video_tx.clone(),
            video_rx,
            video_textures: std::collections::HashMap::new(),
            video_order: std::collections::VecDeque::new(),
            video_playing: false,
            video_started: None,
            video_offset: 0.0,
            video_muted: false,
            groups_asked: std::collections::HashSet::new(),
            show_chat_menu: std::env::var("WA_DESK_MENU").is_ok(),
            settings_section: 0,
            show_chat_actions: false,
            show_new_chat: false,
            new_chat_search: String::new(),
            contacts: Vec::new(),
            contacts_rx: native.contacts,
            audio: crate::audio::Audio::new(),
            tray: crate::tray::Tray::new(&crate::icon::window_icon()),
            call_video: None,
            call_frame: None,
            call_frame_generation: 0,
            call_self_frame: None,
            call_self_source: None,
            sticker_tab: 0,
            sticker_query: String::new(),
            emoji_group: 0,
            emoji_gif: false,
            emoji_textures: std::collections::HashMap::new(),
            show_starred: false,
            show_new_group: false,
            new_group_subject: String::new(),
            new_group_members: std::collections::HashSet::new(),
            show_new_contact: false,
            new_contact_phone: String::new(),
            new_contact_name: String::new(),
            group_add_open: false,
            group_add_members: std::collections::HashSet::new(),
        }
    }

    /// Fill the model with sample chats so the UI can be inspected without a
    /// session (`--preview`, used for screenshots).
    pub fn preview_mode(&mut self) {
        self.model = Model::demo();
        self.preview = true;
        self.show_group = false;
        self.show_status = false;
        self.show_channels = false;
        // `WA_DESK_SETTINGS=1` renders the Settings screen for a screenshot, and
        // `WA_DESK_SETTINGS_SECTION=n` picks the page shown.
        self.show_settings = std::env::var("WA_DESK_SETTINGS").is_ok();
        if let Ok(section) = std::env::var("WA_DESK_SETTINGS_SECTION")
            && let Ok(index) = section.parse::<usize>()
        {
            self.settings_section = index.min(4);
        }
        // `WA_DESK_EMOJI=1` opens the emoji picker for a screenshot.
        // `WA_DESK_STICKERS=<dir>` loads PNGs as favourites and opens the sticker
        // panel, so the official layout can be checked without a session.
        if let Ok(dir) = std::env::var("WA_DESK_STICKERS") {
            let mut favorites = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&dir) {
                let mut paths: Vec<std::path::PathBuf> = entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .and_then(|ext| ext.to_str())
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
                    })
                    .collect();
                paths.sort();
                for (index, path) in paths.into_iter().enumerate() {
                    favorites.push(wa_types::FavoriteStickerView {
                        id: format!("demo-{index}"),
                        path: path.to_string_lossy().into_owned(),
                        width: 128,
                        height: 128,
                    });
                }
            }
            self.model.favorites = favorites;
            self.model.active = Some("1203@g.us".to_string());
            self.model.stickers_open = true;
            self.sticker_tab = 1;
        }
        if std::env::var("WA_DESK_EMOJI").is_ok() {
            self.model.emoji_open = true;
        }
        // `WA_DESK_CALL=ringing|connected|video` paints the call sheet;
        // `WA_DESK_CALL_VIDEO=<file.h264>` feeds the video stage for a screenshot.
        if let Ok(kind) = std::env::var("WA_DESK_CALL") {
            let connected = kind != "ringing";
            let video = kind == "video";
            self.model.call = Some(crate::model::CallState {
                call_id: "demo-call".to_string(),
                peer: "1@s.whatsapp.net".to_string(),
                peer_name: "Ada Lovelace".to_string(),
                state: if connected { "connected" } else { "ringing" }.to_string(),
                video,
                muted: false,
                started_at: connected.then(|| now_ms() - 42_000),
            });
            if video && let Ok(path) = std::env::var("WA_DESK_CALL_SELF") {
                self.call_self_source = Some(path);
            }
            if video
                && let Ok(path) = std::env::var("WA_DESK_CALL_VIDEO")
                && let Ok(bytes) = std::fs::read(path)
                && let Some(decoder) = crate::callvideo::CallVideo::start()
            {
                for chunk in bytes.chunks(4096) {
                    decoder.push(chunk.to_vec());
                }
                self.call_video = Some(decoder);
            }
        }
        // `WA_DESK_VIEWER=1` opens the media viewer on the demo video;
        // `WA_DESK_DEMO_VIDEO=<clip>` supplies a real file to play and
        // `WA_DESK_VIEWER_PLAY=1` starts it so a screenshot shows playback.
        if std::env::var("WA_DESK_VIEWER").is_ok() {
            if let Ok(path) = std::env::var("WA_DESK_DEMO_VIDEO") {
                self.model.media_ready("d8", &path);
            }
            self.preview_autoplay = std::env::var("WA_DESK_VIEWER_PLAY").is_ok();
            self.model.open_viewer("d8");
        }
    }

    /// `--screenshot <path>`: save one settled frame and quit.
    pub fn set_screenshot(&mut self, path: std::path::PathBuf) {
        self.screenshot = Some((path, std::time::Instant::now(), false));
    }

    fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some((path, started, requested)) = self.screenshot.as_mut() else {
            return;
        };
        if !*requested {
            let delay: u64 = std::env::var("WA_DESK_SCREENSHOT_DELAY")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(14);
            if started.elapsed() >= std::time::Duration::from_secs(delay) {
                *requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
        }
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            if let egui::Event::Screenshot { image, .. } = event {
                let size = [image.width() as u32, image.height() as u32];
                let rgba: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                if let Some(buffer) = image::RgbaImage::from_raw(size[0], size[1], rgba) {
                    let _ = buffer.save(&*path);
                }
                std::process::exit(0);
            }
        }
        ctx.request_repaint();
    }

    /// The Chats kebab menu: a small card with the actions the official client
    /// keeps there.
    fn chat_menu(&mut self, ctx: &egui::Context) {
        if !self.show_chat_menu {
            return;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.show_chat_menu = false;
            return;
        }
        let palette = self.palette.clone();
        egui::Window::new("chats-menu")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(egui::pos2(58.0 + 300.0, 54.0))
            .frame(
                egui::Frame::new()
                    .fill(palette.elevated)
                    .stroke(egui::Stroke::new(1.0_f32, palette.border))
                    .corner_radius(egui::CornerRadius::same(12))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(120),
                    })
                    .inner_margin(egui::Margin::symmetric(6, 6)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(230.0);
                // The official Chats menu: New group, Starred messages, Select
                // chats, Mark all as read, then App lock and Log out.
                if menu_row(ui, &palette, Icon::GroupAdd, "New group") {
                    self.show_new_group = true;
                    self.send(Command::Contacts);
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::Star, "Starred messages") {
                    self.show_starred = true;
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::SelectChats, "Select chats") {
                    self.notice = Some((
                        "Selecting several chats is not supported yet.".to_string(),
                        std::time::Instant::now(),
                    ));
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::MarkRead, "Mark all as read") {
                    let unread: Vec<String> = self
                        .model
                        .chats
                        .values()
                        .filter(|row| row.unread > 0)
                        .map(|row| row.jid.clone())
                        .collect();
                    for chat in unread {
                        self.send(Command::MarkRead { chat });
                    }
                    self.show_chat_menu = false;
                }
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
                if menu_row(ui, &palette, Icon::Lock, "App lock") {
                    self.notice = Some((
                        "App lock is not supported yet.".to_string(),
                        std::time::Instant::now(),
                    ));
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::Logout, "Log out") {
                    self.send(Command::Relink);
                    self.show_chat_menu = false;
                }
            });
    }

    /// The open chat's actions menu (mute, pin, unread, clear, delete).
    fn chat_actions_menu(&mut self, ctx: &egui::Context) {
        if !self.show_chat_actions {
            return;
        }
        let Some(chat) = self.model.active.clone() else {
            self.show_chat_actions = false;
            return;
        };
        if let Some(action) = self.chat_actions_window(ctx, &chat, chat.ends_with("@g.us")) {
            if action == "info" {
                self.show_group = true;
                if self.model.request_group(&chat) {
                    self.send(Command::GroupInfo { chat: chat.clone() });
                }
            } else if action == "import" {
                self.import_chat_export(&chat);
            } else if action == "voice" || action == "video" {
                self.start_call(action == "video");
            } else {
                self.send(Command::ChatAction {
                    chat: chat.clone(),
                    action,
                });
            }
            self.show_chat_actions = false;
        }
    }

    fn chat_actions_window(
        &mut self,
        ctx: &egui::Context,
        chat: &str,
        is_group: bool,
    ) -> Option<String> {
        let palette = self.palette.clone();
        let mut chosen = None;
        egui::Window::new("chat-actions")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(egui::pos2(ctx.screen_rect().width() - 300.0, 62.0))
            .frame(
                egui::Frame::new()
                    .fill(palette.elevated)
                    .stroke(egui::Stroke::new(1.0_f32, palette.border))
                    .corner_radius(egui::CornerRadius::same(12))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(120),
                    })
                    .inner_margin(egui::Margin::symmetric(6, 6)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(220.0);
                if is_group && menu_row(ui, &palette, Icon::Groups, "Group info") {
                    chosen = Some("info".to_string());
                }
                // The official menu leads with the calls.
                if !is_group {
                    if menu_row(ui, &palette, Icon::Phone, "Voice call") {
                        chosen = Some("voice".to_string());
                    }
                    if menu_row(ui, &palette, Icon::VideoCall, "Video call") {
                        chosen = Some("video".to_string());
                    }
                }
                for (label, action) in [
                    ("Mute for 8 hours", "mute8h"),
                    ("Mute for 1 week", "mute1w"),
                    ("Unmute", "unmute"),
                    ("Pin chat", "pin"),
                    ("Unpin chat", "unpin"),
                    ("Mark as unread", "unread"),
                    ("Archive chat", "archive"),
                    ("Clear chat", "clear"),
                    ("Delete chat", "delete"),
                    ("Import chat export…", "import"),
                ] {
                    if menu_row(ui, &palette, Icon::Document, label) {
                        chosen = Some(action.to_string());
                    }
                }
            });
        let _ = chat;
        chosen
    }

    /// The new-chat picker: the address book with a search field.
    fn new_chat_window(&mut self, ctx: &egui::Context) {
        if !self.show_new_chat {
            return;
        }
        let palette = self.palette.clone();
        let mut open = true;
        egui::Window::new("New chat")
            .open(&mut open)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                // The official picker leads with the two "create" rows.
                ui.horizontal(|ui| {
                    if menu_row(ui, &palette, Icon::GroupAdd, "New group") {
                        self.show_new_group = true;
                        self.send(Command::Contacts);
                        self.show_new_chat = false;
                    }
                    if menu_row(ui, &palette, Icon::PersonAdd, "New contact") {
                        self.show_new_contact = true;
                        self.show_new_chat = false;
                    }
                });
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_chat_search)
                        .hint_text("Search contacts")
                        .desired_width(f32::INFINITY),
                );
                let needle = self.new_chat_search.trim().to_lowercase();
                let rows: Vec<(String, String)> = self
                    .contacts
                    .iter()
                    .filter(|(_, name)| needle.is_empty() || name.to_lowercase().contains(&needle))
                    .take(80)
                    .cloned()
                    .collect();
                if rows.is_empty() {
                    ui.label(
                        egui::RichText::new("No contacts yet - sync the address book first")
                            .small()
                            .weak(),
                    );
                }
                egui::ScrollArea::vertical()
                    .max_height(380.0)
                    .show(ui, |ui| {
                        for (jid, name) in rows {
                            if ui
                                .add(egui::Button::new(name).frame(false))
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.model.open_chat(&jid);
                                self.send(Command::MarkRead { chat: jid.clone() });
                                self.send(Command::ChatHistory { chat: jid });
                                self.show_new_chat = false;
                            }
                        }
                    });
            });
        let _ = palette;
        if !open {
            self.show_new_chat = false;
        }
    }

    /// Load a WhatsApp "Export chat" file into the open chat.
    fn import_chat_export(&mut self, chat: &str) {
        let picked = rfd::FileDialog::new()
            .add_filter("WhatsApp export", &["txt"])
            .pick_file();
        let Some(path) = picked else { return };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let (added, removed) = self.model.import_export(chat, &text);
                self.cache_dirty = true;
                self.attach_note = Some((
                    format!("Imported {added} message(s), superseded {removed}"),
                    std::time::Instant::now(),
                ));
            }
            Err(err) => {
                self.attach_note =
                    Some((format!("Import failed: {err}"), std::time::Instant::now()));
            }
        }
    }

    /// The starred-messages screen, gathered across chats.
    fn starred_window(&mut self, ctx: &egui::Context) {
        if !self.show_starred {
            return;
        }
        let palette = self.palette.clone();
        let rows = self.model.starred_messages();
        let mut open = true;
        egui::Window::new("Starred messages")
            .open(&mut open)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                if rows.is_empty() {
                    ui.label(egui::RichText::new("No starred messages").weak());
                    return;
                }
                egui::ScrollArea::vertical()
                    .max_height(460.0)
                    .show(ui, |ui| {
                        for (chat, message) in &rows {
                            let name = self
                                .model
                                .chats
                                .get(chat)
                                .map(|row| self.model.title(row))
                                .unwrap_or_else(|| chat.clone());
                            let body = self.model.bubble_body(message);
                            let label = format!(
                                "{name}  ·  {}\n{}",
                                self.model.bubble_stamp(message),
                                body
                            );
                            if ui.add(egui::Button::new(label).frame(false)).clicked() {
                                self.model.open_chat(chat);
                                self.send(Command::MarkRead { chat: chat.clone() });
                                self.show_starred = false;
                            }
                            ui.separator();
                        }
                    });
            });
        let _ = palette;
        if !open {
            self.show_starred = false;
        }
    }

    /// Create a group: a name and a contact checklist.
    fn new_group_window(&mut self, ctx: &egui::Context) {
        if !self.show_new_group {
            return;
        }
        let mut open = true;
        let contacts = self.contacts.clone();
        egui::Window::new("New group")
            .open(&mut open)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_group_subject)
                        .hint_text("Group name")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("{} selected", self.new_group_members.len()))
                        .small()
                        .weak(),
                );
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for (jid, name) in contacts {
                            let mut checked = self.new_group_members.contains(&jid);
                            if ui.checkbox(&mut checked, name).changed() {
                                if checked {
                                    self.new_group_members.insert(jid);
                                } else {
                                    self.new_group_members.remove(&jid);
                                }
                            }
                        }
                    });
                let ready =
                    !self.new_group_subject.trim().is_empty() && !self.new_group_members.is_empty();
                if ui
                    .add_enabled(ready, egui::Button::new("Create group"))
                    .clicked()
                {
                    self.send(Command::CreateGroup {
                        subject: self.new_group_subject.trim().to_string(),
                        members: self.new_group_members.iter().cloned().collect(),
                    });
                    self.new_group_subject.clear();
                    self.new_group_members.clear();
                    self.show_new_group = false;
                }
            });
        if !open {
            self.show_new_group = false;
        }
    }

    /// Add a saved contact (name + international number).
    fn new_contact_window(&mut self, ctx: &egui::Context) {
        if !self.show_new_contact {
            return;
        }
        let mut open = true;
        egui::Window::new("New contact")
            .open(&mut open)
            .resizable(false)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_contact_name)
                        .hint_text("Name")
                        .desired_width(f32::INFINITY),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_contact_phone)
                        .hint_text("628123456789")
                        .desired_width(f32::INFINITY),
                );
                let ready = !self.new_contact_name.trim().is_empty()
                    && !self.new_contact_phone.trim().is_empty();
                if ui.add_enabled(ready, egui::Button::new("Save")).clicked() {
                    self.send(Command::NewContact {
                        phone: self.new_contact_phone.trim().to_string(),
                        name: self.new_contact_name.trim().to_string(),
                    });
                    self.new_contact_phone.clear();
                    self.new_contact_name.clear();
                    self.show_new_contact = false;
                }
            });
        if !open {
            self.show_new_contact = false;
        }
    }

    /// Add members to the open group.
    fn group_add_window(&mut self, ctx: &egui::Context) {
        if !self.group_add_open {
            return;
        }
        let mut open = true;
        let contacts = self.contacts.clone();
        let chat = self.model.active.clone().unwrap_or_default();
        egui::Window::new("Add members")
            .open(&mut open)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for (jid, name) in contacts {
                            let mut checked = self.group_add_members.contains(&jid);
                            if ui.checkbox(&mut checked, name).changed() {
                                if checked {
                                    self.group_add_members.insert(jid);
                                } else {
                                    self.group_add_members.remove(&jid);
                                }
                            }
                        }
                    });
                let count = self.group_add_members.len();
                if ui
                    .add_enabled(
                        count > 0,
                        egui::Button::new(format!("Add {count} member(s)")),
                    )
                    .clicked()
                {
                    let members: Vec<String> = self.group_add_members.iter().cloned().collect();
                    for member in members {
                        self.send(Command::GroupMember {
                            chat: chat.clone(),
                            member,
                            action: "add".into(),
                        });
                    }
                    self.group_add_members.clear();
                    self.group_add_open = false;
                }
            });
        if !open {
            self.group_add_open = false;
        }
    }

    /// Seconds of the clip that have played.
    fn video_elapsed(&self) -> f32 {
        if self.video_playing {
            self.video_started
                .map(|started| started.elapsed().as_secs_f32())
                .unwrap_or(self.video_offset)
        } else {
            self.video_offset
        }
    }

    /// Start or pause the open clip, keeping the extracted audio in step.
    fn toggle_video(&mut self, id: &str) {
        if self.video_playing {
            self.video_offset = self.video_elapsed();
            self.video_playing = false;
            self.video_started = None;
            self.audio.stop();
            return;
        }
        let Some(VideoState::Ready(video)) = self.videos.get(id).cloned() else {
            return;
        };
        if self.video_offset >= video.duration {
            self.video_offset = 0.0;
        }
        self.video_started =
            Some(std::time::Instant::now() - std::time::Duration::from_secs_f32(self.video_offset));
        self.video_playing = true;
        if let Some(audio) = video.audio.clone() {
            self.audio.play_from(
                &format!("video:{id}"),
                &audio.to_string_lossy(),
                video.duration,
                self.video_offset,
            );
        }
    }

    /// Jump the open clip to `seconds`, keeping the extracted audio in step.
    fn seek_video(&mut self, id: &str, seconds: f32) {
        let Some(VideoState::Ready(video)) = self.videos.get(id).cloned() else {
            return;
        };
        let target = seconds.clamp(0.0, video.duration);
        self.video_offset = target;
        if self.video_playing {
            self.video_started =
                Some(std::time::Instant::now() - std::time::Duration::from_secs_f32(target));
            let track = format!("video:{id}");
            let state = self.audio.state();
            if state.id.as_deref() == Some(track.as_str()) && state.playing {
                // Already loaded: move the sink instead of decoding again, so
                // dragging the seek line stays smooth.
                self.audio.seek(target);
            } else if let Some(audio) = video.audio.clone() {
                self.audio
                    .play_from(&track, &audio.to_string_lossy(), video.duration, target);
            }
        }
    }

    fn stop_video(&mut self) {
        if self.video_playing {
            self.audio.stop();
        }
        self.video_playing = false;
        self.video_started = None;
        self.video_offset = 0.0;
    }

    /// The texture for the current frame of a prepared clip.
    fn video_frame_handle(&mut self, ctx: &egui::Context, id: &str) -> Option<egui::TextureHandle> {
        let VideoState::Ready(video) = self.videos.get(id)?.clone() else {
            return None;
        };
        let index = ((self.video_elapsed() * crate::video::FPS) as usize)
            .min(video.frames.saturating_sub(1));
        let key = format!("{}#{index}", video.dir.display());
        // Frames are ~1 MB of texture each, so only a short window is kept:
        // the last ~5 seconds at 10 fps. The rest is re-decoded on demand.
        if let Some(position) = self.video_order.iter().position(|entry| entry == &key) {
            self.video_order.remove(position);
        }
        self.video_order.push_back(key.clone());
        while self.video_order.len() > 48 {
            if let Some(old) = self.video_order.pop_front() {
                self.video_textures.remove(&old);
            }
        }
        let path = video
            .frame_path(index)
            .map(|path| path.to_string_lossy().into_owned());
        texture_at(ctx, &mut self.video_textures, &key, path)
    }

    /// The official video transport bar: play/pause, elapsed, a seek line,
    /// the length, mute and fullscreen.
    fn video_transport(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, id: &str) {
        let duration = match self.videos.get(id) {
            Some(VideoState::Ready(video)) => video.duration,
            _ => 0.0,
        };
        let playing = self.video_playing;
        let elapsed = self.video_elapsed().clamp(0.0, duration.max(0.0));
        let muted = self.video_muted;
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), egui::Sense::hover());
        let white = egui::Color32::from_gray(238);
        let dim = egui::Color32::from_gray(150);
        let middle = rect.center().y;
        let painter = ui.painter();

        let play_rect = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 22.0, middle),
            egui::vec2(38.0, 38.0),
        );
        let play = ui.interact(play_rect, ui.id().with("video-play"), egui::Sense::click());
        painter.circle_filled(
            play_rect.center(),
            19.0,
            if play.hovered() {
                egui::Color32::from_white_alpha(34)
            } else {
                egui::Color32::from_white_alpha(14)
            },
        );
        draw_icon(
            ui,
            play_rect.shrink(8.0),
            if playing { Icon::Pause } else { Icon::Play },
            white,
        );

        let left = rect.left() + 54.0;
        // Mute + fullscreen own the right end; the times sit just before them.
        let full_center = egui::pos2(rect.right() - 26.0, middle);
        let mute_center = egui::pos2(full_center.x - 44.0, middle);
        let times_right = mute_center.x - 30.0;
        let right = (times_right - 96.0).max(left + 40.0);
        let seek_rect = egui::Rect::from_min_max(
            egui::pos2(left, middle - 9.0),
            egui::pos2(right, middle + 9.0),
        );
        let seek = ui.interact(
            seek_rect,
            ui.id().with("video-seek"),
            egui::Sense::click_and_drag(),
        );
        let ratio = if duration > 0.0 {
            (elapsed / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };
        painter.line_segment(
            [egui::pos2(left, middle), egui::pos2(right, middle)],
            egui::Stroke::new(3.0_f32, egui::Color32::from_white_alpha(45)),
        );
        let knob = left + (right - left) * ratio;
        painter.line_segment(
            [egui::pos2(left, middle), egui::pos2(knob, middle)],
            egui::Stroke::new(3.0_f32, egui::Color32::from_rgb(33, 192, 99)),
        );
        painter.circle_filled(
            egui::pos2(knob, middle),
            if seek.hovered() || seek.dragged() {
                7.0
            } else {
                5.5
            },
            white,
        );

        let font = egui::FontId::proportional(12.0);
        let played_label = stamp_seconds(elapsed);
        let total_label = format!("/ {}", stamp_seconds(duration));
        let total_width = painter
            .layout_no_wrap(total_label.clone(), font.clone(), dim)
            .size()
            .x;
        painter.text(
            egui::pos2(times_right, middle),
            egui::Align2::RIGHT_CENTER,
            total_label,
            font.clone(),
            dim,
        );
        painter.text(
            egui::pos2(times_right - total_width - 4.0, middle),
            egui::Align2::RIGHT_CENTER,
            played_label,
            font,
            white,
        );

        let mute_rect = egui::Rect::from_center_size(mute_center, egui::vec2(34.0, 34.0));
        let mute = ui.interact(mute_rect, ui.id().with("video-mute"), egui::Sense::click());
        if mute.hovered() {
            painter.circle_filled(
                mute_rect.center(),
                16.0,
                egui::Color32::from_white_alpha(34),
            );
        }
        draw_icon(
            ui,
            mute_rect.shrink(8.0),
            if muted { Icon::Mute } else { Icon::Volume },
            white,
        );

        let full_rect = egui::Rect::from_center_size(full_center, egui::vec2(34.0, 34.0));
        let full = ui.interact(
            full_rect,
            ui.id().with("video-fullscreen"),
            egui::Sense::click(),
        );
        if full.hovered() {
            painter.circle_filled(
                full_rect.center(),
                16.0,
                egui::Color32::from_white_alpha(34),
            );
        }
        let fullscreen = ctx.input(|input| input.viewport().fullscreen.unwrap_or(false));
        draw_icon(
            ui,
            full_rect.shrink(8.0),
            if fullscreen {
                Icon::FullscreenExit
            } else {
                Icon::Fullscreen
            },
            white,
        );

        if play.clicked() {
            self.toggle_video(id);
        }
        if mute.clicked() {
            self.video_muted = !self.video_muted;
            self.audio
                .set_volume(if self.video_muted { 0.0 } else { 1.0 });
        }
        if full.clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }
        if (seek.clicked() || seek.dragged())
            && let Some(position) = seek.interact_pointer_pos()
            && right > left
        {
            let ratio = ((position.x - left) / (right - left)).clamp(0.0, 1.0);
            self.seek_video(id, ratio * duration);
        }
    }

    /// A toast for a message that arrived while the window is in the
    /// background, like the official client. History replays never get here
    /// (`HistoryMessage` is a different event), and the open chat is skipped.
    fn notify_message(
        &mut self,
        ctx: &egui::Context,
        chat: &str,
        name: &str,
        kind: &str,
        text: &str,
    ) {
        if !self.settings.notifications
            || chat == "status@broadcast"
            || chat.ends_with("@newsletter")
            || self.model.active.as_deref() == Some(chat)
            || ctx.input(|input| input.viewport().focused.unwrap_or(true))
        {
            return;
        }
        let body = notification_body(kind, text);
        let title = if name.trim().is_empty() {
            "WA Desk".to_string()
        } else {
            name.to_string()
        };
        if let Err(err) = crate::notify::send(&title, &body) {
            diag(&format!("notification: {err}"));
        }
    }

    /// System tray: reopen the window or quit.
    fn poll_tray(&mut self, ctx: &egui::Context) {
        let Some(tray) = &self.tray else {
            return;
        };
        match tray.poll() {
            Some(crate::tray::TrayAction::Open) => {
                self.show_status = false;
                self.show_channels = false;
                self.show_group = false;
                self.show_settings = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            Some(crate::tray::TrayAction::Quit) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            None => {}
        }
    }

    /// Start a call in the open chat, with the official guard for groups and
    /// self-chat.
    fn start_call(&mut self, video: bool) {
        let Some(jid) = self.model.active.clone() else {
            return;
        };
        let guard = wa_ui::calls::call_guard(jid.ends_with("@g.us"), self.model.call.is_some());
        if !guard.ok {
            self.notice = guard.toast.map(|toast| (toast, std::time::Instant::now()));
            return;
        }
        self.send(Command::CallStart { chat: jid, video });
    }

    /// The call sheet: ringing (accept/decline) or the live call (mute,
    /// hang-up), with the remote video stage when the call has one.
    fn call_overlay(&mut self, ctx: &egui::Context, call: &crate::model::CallState) {
        let palette = self.palette.clone();
        let now = now_ms();
        let plan = call.overlay(now);
        let status = plan.status(&call.overlay_input(now));
        let screen = ctx.screen_rect();
        let mut action: Option<Command> = None;

        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            action = Some(if plan.ringing {
                Command::CallReject {
                    call_id: call.call_id.clone(),
                }
            } else {
                Command::CallHangup
            });
        }

        egui::Area::new(egui::Id::new("call-overlay"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen.size(), egui::Sense::hover());
                let painter = ui.painter().clone();
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_rgb(11, 20, 26),
                );
                if plan.video_stage {
                    // In a video call the peer's frame fills the sheet; the name
                    // and controls sit over a scrim, like the official client.
                    let stage = rect.shrink(12.0);
                    painter.rect_filled(stage, egui::CornerRadius::same(12), egui::Color32::BLACK);
                    match &self.call_frame {
                        Some(texture) => {
                            let size = texture.size_vec2();
                            let scale = (stage.width() / size.x.max(1.0))
                                .min(stage.height() / size.y.max(1.0));
                            painter.image(
                                texture.id(),
                                egui::Rect::from_center_size(stage.center(), size * scale),
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        }
                        None => {
                            painter.text(
                                stage.center(),
                                egui::Align2::CENTER_CENTER,
                                "Waiting for the peer's video...",
                                egui::FontId::proportional(12.5),
                                egui::Color32::from_gray(150),
                            );
                        }
                    }
                    // Our own camera preview: a mirror thumbnail in the corner,
                    // the way the official client floats the self-view.
                    if let Some(texture) = &self.call_self_frame {
                        let size = texture.size_vec2();
                        let scale = (196.0 / size.x.max(1.0)).min(147.0 / size.y.max(1.0));
                        let thumb = egui::Rect::from_center_size(
                            egui::pos2(
                                stage.right() - 12.0 - size.x * scale / 2.0,
                                stage.bottom() - 12.0 - size.y * scale / 2.0,
                            ),
                            size * scale,
                        );
                        // Mirrored horizontally, like a selfie camera.
                        painter.image(
                            texture.id(),
                            thumb,
                            egui::Rect::from_min_max(egui::pos2(1.0, 0.0), egui::pos2(0.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                }
                self.ensure_avatar(ui.ctx(), &call.peer);
                let avatar = self.avatar_for(&call.peer).cloned();
                let video = plan.video_stage;
                let avatar_radius = if video { 20.0 } else { 48.0 };
                let avatar_center = if video {
                    egui::pos2(
                        rect.left() + 28.0 + avatar_radius,
                        rect.top() + 22.0 + avatar_radius,
                    )
                } else {
                    egui::pos2(rect.center().x, rect.center().y - 70.0)
                };
                match avatar {
                    Some(texture) => {
                        let size = texture.size_vec2();
                        let scale = (avatar_radius * 2.0) / size.x.max(1.0);
                        painter.image(
                            texture.id(),
                            egui::Rect::from_center_size(avatar_center, size * scale),
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                    None => {
                        painter.circle_filled(avatar_center, avatar_radius, palette.selected);
                        painter.text(
                            avatar_center,
                            egui::Align2::CENTER_CENTER,
                            call_initials(&call.peer_name),
                            egui::FontId::proportional(avatar_radius * 0.7),
                            palette.text,
                        );
                    }
                }
                if video {
                    painter.text(
                        egui::pos2(
                            avatar_center.x + avatar_radius + 12.0,
                            avatar_center.y - 9.0,
                        ),
                        egui::Align2::LEFT_CENTER,
                        &call.peer_name,
                        egui::FontId::proportional(16.0),
                        palette.text,
                    );
                    painter.text(
                        egui::pos2(
                            avatar_center.x + avatar_radius + 12.0,
                            avatar_center.y + 11.0,
                        ),
                        egui::Align2::LEFT_CENTER,
                        &status,
                        egui::FontId::proportional(12.5),
                        palette.secondary,
                    );
                } else {
                    painter.text(
                        egui::pos2(rect.center().x, avatar_center.y + avatar_radius + 22.0),
                        egui::Align2::CENTER_CENTER,
                        &call.peer_name,
                        egui::FontId::proportional(20.0),
                        palette.text,
                    );
                    painter.text(
                        egui::pos2(rect.center().x, avatar_center.y + avatar_radius + 50.0),
                        egui::Align2::CENTER_CENTER,
                        &status,
                        egui::FontId::proportional(13.0),
                        palette.secondary,
                    );
                }

                // Controls.
                let buttons_y = if video {
                    rect.bottom() - 86.0
                } else {
                    avatar_center.y + 190.0
                };
                let left = egui::pos2(rect.center().x - 62.0, buttons_y);
                let right = egui::pos2(rect.center().x + 62.0, buttons_y);
                let button = |center: egui::Pos2,
                              icon: Icon,
                              background: egui::Color32,
                              foreground: egui::Color32,
                              id: &str,
                              slash: bool|
                 -> bool {
                    let hit_rect = egui::Rect::from_center_size(center, egui::vec2(60.0, 60.0));
                    let response = ui.interact(hit_rect, egui::Id::new(id), egui::Sense::click());
                    painter.circle_filled(
                        center,
                        30.0,
                        if response.hovered() {
                            background.gamma_multiply(1.15)
                        } else {
                            background
                        },
                    );
                    draw_icon(ui, hit_rect.shrink(17.0), icon, foreground);
                    if slash {
                        painter.line_segment(
                            [
                                egui::pos2(center.x - 13.0, center.y + 13.0),
                                egui::pos2(center.x + 13.0, center.y - 13.0),
                            ],
                            egui::Stroke::new(2.5_f32, foreground),
                        );
                    }
                    response.clicked()
                };

                if plan.ringing {
                    let decline = button(
                        left,
                        Icon::CallEnd,
                        palette.danger,
                        egui::Color32::WHITE,
                        "call-decline",
                        false,
                    );
                    let accept = button(
                        right,
                        Icon::Phone,
                        palette.accent,
                        egui::Color32::from_rgb(11, 20, 26),
                        "call-accept",
                        false,
                    );
                    if decline {
                        action = Some(Command::CallReject {
                            call_id: call.call_id.clone(),
                        });
                    }
                    if accept {
                        action = Some(Command::CallAccept {
                            call_id: call.call_id.clone(),
                            video: call.video,
                        });
                    }
                } else {
                    let muted = plan.mute_pressed;
                    let mute = button(
                        left,
                        Icon::Mic,
                        if muted {
                            palette.danger
                        } else {
                            palette.elevated
                        },
                        egui::Color32::WHITE,
                        "call-mute",
                        muted,
                    );
                    let hangup = button(
                        right,
                        Icon::CallEnd,
                        palette.danger,
                        egui::Color32::WHITE,
                        "call-hangup",
                        false,
                    );
                    if mute {
                        action = Some(Command::CallMute { muted: !call.muted });
                    }
                    if hangup {
                        action = Some(Command::CallHangup);
                    }
                }
            });

        if let Some(command) = action {
            // Reflect the intent immediately; the session confirms with events.
            if let Command::CallMute { muted } = &command
                && let Some(state) = self.model.call.as_mut()
            {
                state.muted = *muted;
            }
            self.send(command);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    /// Check the release list on a worker thread; the window never blocks.
    fn check_updates(&mut self) {
        self.update = UpdateState::Checking;
        let tx = self.update_tx.clone();
        std::thread::spawn(move || {
            let reply = updates::check(env!("CARGO_PKG_VERSION"));
            let _ = tx.send(UpdateMsg::Checked(reply));
        });
    }

    /// Download the release binary on a worker thread.
    fn download_update(&mut self, release: Release) {
        self.update = UpdateState::Downloading(release.clone());
        let tx = self.update_tx.clone();
        std::thread::spawn(move || {
            let reply = updates::download(&release, &updates::staging_path());
            let _ = tx.send(UpdateMsg::Downloaded(reply));
        });
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            crate::app::diag(&format!("event {}", event.kind()));
            let incoming = match &event {
                UiEvent::Message(message) if !message.from_me => Some((
                    message.chat.clone(),
                    message.chat_name.clone(),
                    message.kind.clone(),
                    message.text.clone(),
                )),
                UiEvent::CallAudio { samples, .. } => {
                    self.audio.play_call_audio(samples.clone());
                    None
                }
                UiEvent::CallSelfVideo {
                    data,
                    width,
                    height,
                } => {
                    use base64::Engine as _;
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data)
                        && bytes.len() == (*width as usize) * (*height as usize) * 4
                    {
                        let image = egui::ColorImage::from_rgba_unmultiplied(
                            [*width as usize, *height as usize],
                            &bytes,
                        );
                        self.call_self_frame = Some(ctx.load_texture(
                            "call-self-frame",
                            image,
                            egui::TextureOptions::LINEAR,
                        ));
                    }
                    None
                }
                UiEvent::CallVideo { data, .. } => {
                    // Annex-B access units from the peer: decode with ffmpeg and
                    // paint the newest frame in the call stage.
                    use base64::Engine as _;
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data) {
                        if self.call_video.is_none() {
                            self.call_video = crate::callvideo::CallVideo::start();
                        }
                        if let Some(decoder) = &self.call_video {
                            decoder.push(bytes);
                        }
                    }
                    None
                }
                UiEvent::CallEnded { .. } => {
                    self.audio.stop_call();
                    self.call_video = None;
                    self.call_frame = None;
                    self.call_frame_generation = 0;
                    None
                }
                _ => None,
            };
            self.model.apply(&event);
            changed = true;
            if let Some((chat, name, kind, text)) = incoming {
                self.notify_message(ctx, &chat, &name, &kind, &text);
            }
        }
        while let Ok((chat, reply)) = self.group_rx.try_recv() {
            match reply {
                Ok(info) => self.model.group_loaded(info),
                Err(err) => self.model.group_failed(&err),
            }
            let _ = chat;
            changed = true;
        }
        while let Ok(reply) = self.favorites_rx.try_recv() {
            match reply {
                Ok(favorites) => self.model.favorites_loaded(favorites),
                Err(err) => self.model.favorites_failed(&err),
            }
            changed = true;
        }
        while let Ok(message) = self.update_rx.try_recv() {
            self.update = match message {
                UpdateMsg::Checked(Ok(Some(release))) => UpdateState::Available(release),
                UpdateMsg::Checked(Ok(None)) => UpdateState::UpToDate,
                UpdateMsg::Checked(Err(err)) => UpdateState::Failed(err),
                UpdateMsg::Downloaded(Ok(path)) => UpdateState::Ready(path),
                UpdateMsg::Downloaded(Err(err)) => UpdateState::Failed(err),
            };
            changed = true;
        }
        while let Ok((jid, url)) = self.avatar_rx.try_recv() {
            match url {
                Some(url) => {
                    let tx = self.avatar_img_tx.clone();
                    std::thread::spawn(move || {
                        let result = crate::updates::fetch_bytes(&url);
                        crate::app::diag(&format!(
                            "avatar download {url}: {}",
                            match &result {
                                Ok(bytes) => format!("{} bytes", bytes.len()),
                                Err(err) => err.clone(),
                            }
                        ));
                        let _ = tx.send((jid, result));
                    });
                }
                None => {
                    self.avatars.insert(jid, None);
                }
            }
            changed = true;
        }
        while let Ok((jid, bytes)) = self.avatar_img_rx.try_recv() {
            if let Ok(bytes) = &bytes {
                // Cache the photo so the next launch paints it instantly.
                let _ = std::fs::write(crate::model::avatar_path(&jid), bytes);
            }
            let texture = bytes
                .ok()
                .and_then(|bytes| circular_avatar(ctx, &format!("avatar-{jid}"), &bytes));
            crate::app::diag(&format!(
                "avatar texture {jid}: {}",
                if texture.is_some() { "ok" } else { "failed" }
            ));
            self.avatars.insert(jid, texture);
            changed = true;
        }
        while let Ok(list) = self.contacts_rx.try_recv() {
            self.contacts = list;
            changed = true;
        }
        while let Ok(result) = self.attach_rx.try_recv() {
            let note = match result {
                Ok(note) => note,
                Err(err) => err,
            };
            self.attach_note = Some((note, std::time::Instant::now()));
            changed = true;
        }
        while let Ok((id, reply)) = self.video_rx.try_recv() {
            diag(&format!(
                "video {id}: {}",
                match &reply {
                    Ok(video) => format!("{} frames", video.frames),
                    Err(err) => err.clone(),
                }
            ));
            self.videos.insert(
                id,
                match reply {
                    Ok(video) => VideoState::Ready(video),
                    Err(err) => VideoState::Failed(err),
                },
            );
            changed = true;
        }
        while let Ok((id, reply)) = self.media_rx.try_recv() {
            match reply {
                Ok(path) => self.model.media_ready(&id, &path),
                Err(_) => self.model.media_failed(&id),
            }
            changed = true;
        }
        if changed {
            self.cache_dirty = true;
            ctx.request_repaint();
        }
    }

    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// The paperclip: pick a file and send it to the open chat.
    fn pick_attachment(&mut self) {
        let Some(chat) = self.model.active.clone() else {
            return;
        };
        let picked = rfd::FileDialog::new().set_title("Send a file").pick_file();
        let Some(path) = picked else { return };
        let text = self.model.composer.trim().to_string();
        let caption = (!text.is_empty()).then_some(text);
        if caption.is_some() {
            self.model.composer.clear();
        }
        self.send_document(&chat, path, caption);
    }

    /// Queue one file for upload and show what is happening under the composer.
    fn send_document(&mut self, chat: &str, path: std::path::PathBuf, caption: Option<String>) {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".to_string());
        self.attach_note = Some((format!("Sending {name}…"), std::time::Instant::now()));
        self.send(Command::SendDocument {
            chat: chat.to_string(),
            path: path.to_string_lossy().into_owned(),
            caption,
        });
    }

    /// Ask for a contact's photo once; `paint_avatar` falls back to initials.
    fn ensure_avatar(&mut self, ctx: &egui::Context, jid: &str) {
        if jid.is_empty() {
            return;
        }
        if !self.avatar_asked.contains(jid) {
            self.avatar_asked.insert(jid.to_string());
            self.send(Command::Avatar {
                jid: jid.to_string(),
            });
        }
        // Show the cached file instantly; the network refresh replaces it.
        if !self.avatars.contains_key(jid)
            && self.avatar_disk.insert(jid.to_string())
            && let Ok(bytes) = std::fs::read(crate::model::avatar_path(jid))
        {
            let texture = circular_avatar(ctx, &format!("avatar-{jid}"), &bytes);
            self.avatars.insert(jid.to_string(), texture);
        }
    }

    fn avatar_for(&self, jid: &str) -> Option<&egui::TextureHandle> {
        self.avatars.get(jid).and_then(|entry| entry.as_ref())
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();

        // The navigation rail, like the official client.
        egui::SidePanel::left("rail")
            .resizable(false)
            .exact_width(58.0)
            .frame(egui::Frame::new().fill(palette.rail))
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    let chats = !self.show_status && !self.show_channels && !self.show_settings;
                    let community_view =
                        chats && self.model.chat_filter == crate::model::ChatFilter::Communities;
                    if rail_button(ui, &palette, Icon::Chats, chats, self.model.total_unread())
                        .on_hover_text("Chats")
                        .clicked()
                    {
                        self.show_status = false;
                        self.show_channels = false;
                        self.show_settings = false;
                        self.model.close_status();
                    }
                    let unseen = self
                        .model
                        .status_groups()
                        .iter()
                        .filter(|group| group.unseen)
                        .count() as i64;
                    if rail_button(ui, &palette, Icon::Status, self.show_status, unseen)
                        .on_hover_text("Status")
                        .clicked()
                    {
                        self.show_status = true;
                        self.show_channels = false;
                        self.show_settings = false;
                        self.model.close_status();
                        self.send(Command::StatusHistory);
                    }
                    if rail_button(ui, &palette, Icon::Channels, self.show_channels, 0)
                        .on_hover_text("Channels")
                        .clicked()
                    {
                        self.show_channels = true;
                        self.show_status = false;
                        self.show_settings = false;
                        self.model.close_status();
                    }
                    if rail_button(ui, &palette, Icon::Groups, community_view, 0)
                        .on_hover_text("Communities")
                        .clicked()
                    {
                        self.show_channels = false;
                        self.show_status = false;
                        self.show_settings = false;
                        self.model.chat_filter = crate::model::ChatFilter::Communities;
                    }
                    if rail_button(ui, &palette, Icon::Settings, self.show_settings, 0)
                        .on_hover_text("Settings")
                        .clicked()
                    {
                        self.show_settings = true;
                        self.show_channels = false;
                        self.show_status = false;
                        self.model.close_status();
                    }
                });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    self.ensure_avatar(ui.ctx(), &self.model.me_jid.clone());
                    let own_avatar = self.avatar_for(&self.model.me_jid.clone()).cloned();
                    paint_avatar(ui, &self.model.own_name(), 30.0, own_avatar.as_ref());
                });
            });

        // Settings replaces the chat list, like the official client: the section
        // list takes this column instead.
        if self.show_settings {
            return;
        }

        egui::SidePanel::left("chats")
            .resizable(true)
            .default_width(368.0)
            .show(ctx, |ui| {
                let palette = self.palette.clone();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    self.ensure_avatar(ui.ctx(), &self.model.me_jid.clone());
                    let own_avatar = self.avatar_for(&self.model.me_jid.clone()).cloned();
                    paint_avatar(ui, &self.model.own_name(), 34.0, own_avatar.as_ref());
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("Chats").size(19.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if fab_button(ui, &palette).on_hover_text("New chat").clicked() {
                            self.show_new_chat = true;
                            self.send(Command::Contacts);
                        }
                        if icon_button(ui, Icon::Menu, &palette)
                            .on_hover_text("Menu")
                            .clicked()
                        {
                            // The sidebar kebab opens the Chats menu - not the
                            // open chat's menu (that one lives in the header).
                            self.show_chat_actions = false;
                            self.show_chat_menu = !self.show_chat_menu;
                        }
                        if matches!(
                            self.update,
                            UpdateState::Available(_) | UpdateState::Ready(_)
                        ) {
                            ui.label(egui::RichText::new("⬆").color(palette.accent).size(16.0))
                                .on_hover_text("An update is ready - see Settings → Updates");
                        }
                    });
                });
                ui.add_space(8.0);
                // The rounded search field.
                egui::Frame::new()
                    .fill(palette.rail)
                    .corner_radius(egui::CornerRadius::same(9))
                    .inner_margin(egui::Margin::symmetric(10, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            draw_icon(ui, rect, Icon::Search, palette.secondary);
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut self.model.search)
                                    .hint_text("Search or start a new chat")
                                    .frame(false)
                                    .desired_width(f32::INFINITY),
                            );
                            if self.focus_search {
                                edit.request_focus();
                                self.focus_search = false;
                            }
                        });
                    });
                ui.add_space(6.0);
                // The filter pills with their counts.
                let (groups, communities, unread) = self.model.filter_counts();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (filter, label) in [
                        (crate::model::ChatFilter::All, "All".to_string()),
                        (crate::model::ChatFilter::Groups, format!("Groups {groups}")),
                        (
                            crate::model::ChatFilter::Communities,
                            format!("Communities {communities}"),
                        ),
                        (crate::model::ChatFilter::Unread, format!("Unread {unread}")),
                    ] {
                        let selected = self.model.chat_filter == filter;
                        if pill_button(ui, &palette, &label, selected) {
                            self.model.chat_filter = filter;
                        }
                    }
                });
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(&self.model.status)
                        .size(11.5)
                        .color(palette.secondary),
                );
                ui.add_space(2.0);

                let rows: Vec<ChatRow> = self.model.visible_chats().into_iter().cloned().collect();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for row in &rows {
                            let selected = self.model.active.as_deref() == Some(row.jid.as_str());
                            let title = self.model.title(row);
                            let preview = self.model.row_preview(row);
                            let stamp = self.model.row_stamp(row);
                            if row.jid.ends_with("@g.us")
                                && row.name.is_empty()
                                && self.groups_asked.insert(row.jid.clone())
                            {
                                self.send(Command::GroupInfo {
                                    chat: row.jid.clone(),
                                });
                            }
                            self.ensure_avatar(ui.ctx(), &row.jid);
                            let avatar = self.avatar_for(&row.jid).cloned();
                            if chat_row(
                                ui,
                                &palette,
                                row,
                                &title,
                                &preview,
                                &stamp,
                                selected,
                                avatar.as_ref(),
                            ) {
                                let jid = row.jid.clone();
                                let empty = row.messages.is_empty();
                                self.audio.stop();
                                self.model.open_chat(&jid);
                                self.send(Command::MarkRead { chat: jid.clone() });
                                if empty {
                                    // WhatsApp does not replay history to an
                                    // existing device; opening a chat asks.
                                    self.send(Command::ChatHistory { chat: jid });
                                }
                            }
                        }
                    });
            });
    }

    fn conversation(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.wallpaper))
            .show(ctx, |ui| {
                let Some(jid) = self.model.active.clone() else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(70.0);
                        ui.horizontal(|ui| {
                            ui.add_space((ui.available_width() / 2.0 - 120.0).max(0.0));
                            if action_button(ui, &palette, Icon::Document, "Send document") {
                                self.focus_search = true;
                            }
                            if action_button(ui, &palette, Icon::Add, "New chat") {
                                self.focus_search = true;
                            }
                            if action_button(ui, &palette, Icon::Settings, "Settings") {
                                self.show_settings = true;
                            }
                        });
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new("Pick a chat on the left").color(palette.secondary),
                        );
                    });
                    return;
                };
                let Some(row) = self.model.chats.get(&jid).cloned() else {
                    return;
                };

                // The chat header: avatar, name, presence, and the two icons the
                // official client keeps (search and the kebab).
                egui::Frame::new()
                    .fill(palette.panel)
                    .inner_margin(egui::Margin::symmetric(14, 7))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            self.ensure_avatar(ui.ctx(), &row.jid);
                            let chat_avatar = self.avatar_for(&row.jid).cloned();
                            paint_avatar(ui, &self.model.title(&row), 38.0, chat_avatar.as_ref());
                            ui.add_space(2.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(self.model.title(&row))
                                        .size(15.5)
                                        .strong(),
                                );
                                let subtitle = if row.jid.ends_with("@g.us") {
                                    "Group"
                                } else {
                                    "online"
                                };
                                ui.label(
                                    egui::RichText::new(subtitle)
                                        .size(11.5)
                                        .color(palette.secondary),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Right to left: the kebab sits at the right edge,
                                    // then search, then the calls - the official order.
                                    if icon_button(ui, Icon::Menu, &palette)
                                        .on_hover_text("Menu")
                                        .clicked()
                                    {
                                        self.show_chat_menu = false;
                                        self.show_chat_actions = !self.show_chat_actions;
                                    }
                                    if icon_button(ui, Icon::Search, &palette)
                                        .on_hover_text("Search")
                                        .clicked()
                                    {
                                        self.model.search_open = !self.model.search_open;
                                        if !self.model.search_open {
                                            self.model.chat_search.clear();
                                            self.model.search_hit = -1;
                                        }
                                    }
                                    if icon_button(ui, Icon::Phone, &palette)
                                        .on_hover_text("Voice call")
                                        .clicked()
                                    {
                                        self.start_call(false);
                                    }
                                    if icon_button(ui, Icon::VideoCall, &palette)
                                        .on_hover_text("Video call")
                                        .clicked()
                                    {
                                        self.start_call(true);
                                    }
                                },
                            );
                        });
                    });

                // In-chat search: the counter, the stepper, Esc to close.
                if self.model.search_open {
                    ui.horizontal(|ui| {
                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut self.model.chat_search)
                                .hint_text("Search this chat")
                                .desired_width(220.0),
                        );
                        if edit.changed() {
                            self.model.search_refresh();
                            self.scrolled_hit = -1;
                        }
                        ui.label(
                            egui::RichText::new(self.model.search_label())
                                .small()
                                .color(self.palette.read),
                        );
                        if ui.button("▲").clicked() {
                            self.model.search_step(false);
                            self.scrolled_hit = -1;
                        }
                        if ui.button("▼").clicked() {
                            self.model.search_step(true);
                            self.scrolled_hit = -1;
                        }
                        if ui.button("✕").clicked() {
                            self.model.search_open = false;
                            self.model.chat_search.clear();
                            self.model.search_hit = -1;
                        }
                    });
                }
                ui.separator();

                // Ask for photos and stickers that are not on disk yet (one
                // request each). Only the newest few: older bubbles were
                // requested when they were on screen.
                let wanted: Vec<String> = row
                    .messages
                    .iter()
                    .rev()
                    .filter(|message| {
                        matches!(message.kind.as_str(), "image" | "sticker" | "audio")
                    })
                    .take(24)
                    .map(|message| message.id.clone())
                    .collect();
                for id in wanted {
                    if self.model.request_media(&id) {
                        self.send(Command::FetchMedia { id });
                    }
                }

                // What the search marks, and which hit to bring into view.
                let hits: Vec<usize> = if self.model.search_open {
                    self.model.search_matches()
                } else {
                    Vec::new()
                };
                let current = self.model.search_current();
                let query = self.model.chat_search.clone();
                let focus = current.filter(|_| self.scrolled_hit != self.model.search_hit);

                let height = ui.available_height() - 64.0;
                // WhatsApp anchors a short conversation to the composer; the
                // previous frame's content height tells us how much to pad.
                let content_height: f32 = ctx.data(|data| {
                    data.get_temp(egui::Id::new("chat-content-height"))
                        .unwrap_or(0.0)
                });
                let spare = (height - content_height).max(0.0);
                let mut chosen: Option<BubbleAction> = None;
                let playback = self.audio.state();
                egui::ScrollArea::vertical()
                    .stick_to_bottom(focus.is_none())
                    .auto_shrink([false, false])
                    .max_height(height.max(120.0))
                    .show(ui, |ui| {
                        let ctx = ui.ctx().clone();
                        if spare > 0.0 {
                            ui.add_space(spare);
                        }
                        for (index, message) in row.messages.iter().enumerate() {
                            let is_hit = hits.contains(&index);
                            if let Some(action) = bubble(
                                ui,
                                &palette,
                                &self.model,
                                message,
                                &mut self.textures,
                                &ctx,
                                if is_hit { Some(query.as_str()) } else { None },
                                focus == Some(index),
                                &playback,
                            ) {
                                chosen = Some(action);
                            }
                        }
                        ctx.data_mut(|data| {
                            data.insert_temp(
                                egui::Id::new("chat-content-height"),
                                (ui.min_rect().height() - spare).max(0.0),
                            );
                        });
                    });
                if let Some(action) = chosen {
                    self.run_action(&row, action);
                }
                if focus.is_some() {
                    self.scrolled_hit = self.model.search_hit;
                }

                // Esc leaves the search bar, like WhatsApp Web.
                if self.model.search_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.model.search_open = false;
                    self.model.chat_search.clear();
                    self.model.search_hit = -1;
                }

                ui.separator();

                // The reply / edit card above the composer.
                self.composer_banner(ui);

                egui::Frame::new()
                    .fill(palette.panel)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        if let Some((note, at)) = self.attach_note.clone() {
                            if at.elapsed() > std::time::Duration::from_secs(8) {
                                self.attach_note = None;
                            } else {
                                ui.label(
                                    egui::RichText::new(note)
                                        .size(11.5)
                                        .color(palette.secondary),
                                );
                            }
                        }
                        // Ctrl+V with a file on the OS clipboard attaches it.
                        let paste_files = ui.ctx().input(|input| {
                            input.modifiers.command && input.key_pressed(egui::Key::V)
                        });
                        if paste_files && let Some(path) = crate::clipboard::first_file() {
                            let text = self.model.composer.trim().to_string();
                            let caption = (!text.is_empty()).then_some(text);
                            if caption.is_some() {
                                self.model.composer.clear();
                            }
                            self.send_document(&jid, path, caption);
                        }
                        ui.horizontal(|ui| {
                            if icon_button(ui, Icon::Attach, &palette)
                                .on_hover_text("Attach a file")
                                .clicked()
                            {
                                self.pick_attachment();
                            }
                            if icon_button(ui, Icon::Emoji, &palette)
                                .on_hover_text("Emoji")
                                .clicked()
                            {
                                self.model.emoji_open = !self.model.emoji_open;
                                self.model.stickers_open = false;
                            }
                            if icon_button(ui, Icon::Sticker, &palette)
                                .on_hover_text("Stickers")
                                .clicked()
                            {
                                self.model.stickers_open = !self.model.stickers_open;
                                self.model.emoji_open = false;
                                if self.model.favorites.is_empty()
                                    && self.model.favorites_error.is_none()
                                {
                                    self.send(Command::FavoriteStickers);
                                }
                            }
                            let send_width = 44.0;
                            let input_width = ui.available_width() - send_width;
                            let edit = egui::Frame::new()
                                .fill(palette.elevated)
                                .corner_radius(egui::CornerRadius::same(18))
                                .inner_margin(egui::Margin::symmetric(10, 3))
                                .show(ui, |ui| {
                                    ui.add_sized(
                                        [(input_width - 20.0).max(80.0), 28.0],
                                        egui::TextEdit::singleline(&mut self.model.composer)
                                            .frame(false)
                                            .hint_text("Type a message"),
                                    )
                                })
                                .inner;
                            if edit.changed() {
                                let typing = !self.model.composer.trim().is_empty();
                                if typing != self.typing_sent {
                                    self.typing_sent = typing;
                                    self.send(Command::Typing {
                                        chat: jid.clone(),
                                        typing,
                                    });
                                }
                            }
                            let enter =
                                edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if icon_button(ui, Icon::Send, &palette)
                                .on_hover_text(self.model.send_label())
                                .clicked()
                                || enter
                            {
                                let text = self.model.composer.trim().to_string();
                                if !text.is_empty() {
                                    if let Some(editing) = self.model.editing.take() {
                                        self.send(Command::EditMessage {
                                            chat: jid.clone(),
                                            id: editing.id,
                                            text,
                                        });
                                    } else if let Some(reply) = self.model.reply_to.take() {
                                        let quoted = if reply.text.trim().is_empty() {
                                            None
                                        } else {
                                            Some(reply.text.clone())
                                        };
                                        self.send(Command::SendReply {
                                            chat: jid.clone(),
                                            text,
                                            reply_id: reply.id,
                                            quoted,
                                        });
                                    } else {
                                        self.send(Command::SendText {
                                            chat: jid.clone(),
                                            text,
                                        });
                                    }
                                    self.model.composer.clear();
                                    self.send(Command::Typing {
                                        chat: jid.clone(),
                                        typing: false,
                                    });
                                    self.typing_sent = false;
                                }
                            }
                        });
                    });
            });
    }

    /// Run what a bubble's context menu asked for.
    fn run_action(&mut self, row: &ChatRow, action: BubbleAction) {
        let id = match &action {
            BubbleAction::OpenMedia(id)
            | BubbleAction::Reply(id)
            | BubbleAction::Edit(id)
            | BubbleAction::Delete(id)
            | BubbleAction::Star(id)
            | BubbleAction::PlayAudio(id)
            | BubbleAction::Forward(id) => id.clone(),
            BubbleAction::React(id, _) => id.clone(),
        };
        let message = row.messages.iter().find(|item| item.id == id).cloned();
        match action {
            BubbleAction::OpenMedia(id) => {
                self.model.open_viewer(&id);
            }
            BubbleAction::Reply(_) => {
                if let Some(message) = message {
                    self.model.start_reply(&message);
                }
            }
            BubbleAction::React(_, emoji) => {
                if let Some(message) = message {
                    self.send(Command::React {
                        chat: message.chat.clone(),
                        id: message.id.clone(),
                        from_me: message.from_me,
                        participant: if message.from_me {
                            None
                        } else {
                            message.sender.clone()
                        },
                        emoji: emoji.clone(),
                    });
                    // Show our own pill straight away; the echo replaces it.
                    self.model.apply_own_reaction(&message.id, &emoji);
                }
            }
            BubbleAction::Edit(_) => {
                if let Some(message) = message {
                    self.model.start_edit(&message);
                }
            }
            BubbleAction::Delete(_) => {
                if let Some(message) = message {
                    self.model.start_delete(&message);
                }
            }
            BubbleAction::Forward(_) => {
                if let Some(message) = message {
                    self.model.start_forward(&message);
                }
            }
            BubbleAction::PlayAudio(id) => {
                if let Some(message) = row.messages.iter().find(|item| item.id == id).cloned() {
                    if let Some(path) = self.model.media_path(&id) {
                        let duration = message
                            .audio
                            .as_ref()
                            .map(|audio| audio.seconds as f32)
                            .unwrap_or(0.0);
                        self.audio.toggle(&id, &path, duration);
                    } else if self.model.request_media(&id) {
                        self.send(Command::FetchMedia { id: id.clone() });
                        self.attach_note = Some((
                            "Loading the voice note…".to_string(),
                            std::time::Instant::now(),
                        ));
                    }
                }
            }
            BubbleAction::Star(id) => {
                if let Some(message) = message {
                    let starred = self.model.toggle_star(&id);
                    self.send(Command::Star {
                        chat: message.chat.clone(),
                        id: message.id.clone(),
                        starred,
                        from_me: message.from_me,
                        participant: if message.from_me {
                            None
                        } else {
                            message.sender.clone()
                        },
                    });
                }
            }
        }
    }

    /// The reply / edit card above the composer, with its cancel button.
    fn composer_banner(&mut self, ui: &mut egui::Ui) {
        if let Some(preview) = self.model.reply_preview() {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(32, 34, 34))
                .inner_margin(egui::Margin::symmetric(8, 4))
                .corner_radius(egui::CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(preview.name)
                                    .small()
                                    .color(egui::Color32::from_rgb(83, 189, 235)),
                            );
                            ui.label(egui::RichText::new(preview.preview).small().weak());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✕").clicked() {
                                self.model.reply_to = None;
                            }
                        });
                    });
                });
            return;
        }
        if let Some(editing) = self.model.editing.clone() {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(32, 34, 34))
                .inner_margin(egui::Margin::symmetric(8, 4))
                .corner_radius(egui::CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Editing message")
                                    .small()
                                    .color(egui::Color32::from_rgb(240, 200, 60)),
                            );
                            ui.label(egui::RichText::new(&editing.text).small().weak());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✕").clicked() {
                                self.model.cancel_composer_mode();
                            }
                        });
                    });
                });
        }
    }
}

/// Paint one message. Returns the action its context menu chose, or a viewer
/// request when its media was clicked. `search` marks a hit (and highlights the
/// query), `scroll` brings the bubble into view.
#[allow(clippy::too_many_arguments)]
fn bubble(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    model: &Model,
    message: &MessageView,
    textures: &mut std::collections::HashMap<String, Option<egui::TextureHandle>>,
    ctx: &egui::Context,
    search: Option<&str>,
    scroll: bool,
    playback: &crate::audio::PlaybackState,
) -> Option<BubbleAction> {
    let align = if message.from_me {
        egui::Align::Max
    } else {
        egui::Align::Min
    };
    let mut action: Option<BubbleAction> = None;
    let max_width = (ui.available_width() * 0.66).max(160.0);

    ui.with_layout(egui::Layout::top_down(align), |ui| {
        ui.set_max_width(max_width);

        // A sticker paints on its own, without a bubble.
        if message.kind == "sticker"
            && let Some(texture) =
                texture_at(ctx, textures, &message.id, model.media_path(&message.id))
        {
            let size = texture.size_vec2();
            let scale = (128.0 / size.x).min(1.0);
            let response =
                ui.add(egui::Image::new((texture.id(), size * scale)).sense(egui::Sense::click()));
            if scroll {
                response.scroll_to_me(Some(egui::Align::Center));
            }
            if response.clicked() {
                action = Some(BubbleAction::OpenMedia(message.id.clone()));
            }
            context_menu(&response, palette, model, message, &mut action);
            ui.label(
                egui::RichText::new(model.bubble_stamp(message))
                    .size(11.0)
                    .color(palette.secondary),
            );
            pills_row(ui, model, message);
            ui.add_space(4.0);
            return;
        }

        // A photo paints its bytes once wa-core has the file; a video shows the
        // same texture as its poster. Anything else falls back to the caption or
        // the kind's word from wa-ui.
        if matches!(message.kind.as_str(), "image" | "video")
            && let Some(texture) =
                texture_at(ctx, textures, &message.id, model.media_path(&message.id))
        {
            let size = texture.size_vec2();
            let scale = (240.0 / size.x).min(1.0);
            let response =
                ui.add(egui::Image::new((texture.id(), size * scale)).sense(egui::Sense::click()));
            if scroll {
                response.scroll_to_me(Some(egui::Align::Center));
            }
            if response.clicked() {
                action = Some(BubbleAction::OpenMedia(message.id.clone()));
            }
            context_menu(&response, palette, model, message, &mut action);
            if !message.text.trim().is_empty() {
                ui.label(
                    egui::RichText::new(&message.text)
                        .size(14.0)
                        .color(palette.text),
                );
            }
            ui.label(
                egui::RichText::new(model.bubble_stamp(message))
                    .size(11.0)
                    .color(palette.secondary),
            );
            pills_row(ui, model, message);
            ui.add_space(4.0);
            return;
        }

        // A document card: file icon, name, size and pages, then the caption.
        if message.kind == "document"
            && let Some(document) = message.document.as_ref()
        {
            let width = max_width.min(310.0);
            let pad = 10.0;
            let icon = 38.0;
            let text_width = (width - pad * 3.0 - icon).max(80.0);
            let name = if document.name.trim().is_empty() {
                model.bubble_body(message)
            } else {
                document.name.clone()
            };
            let caption = if document.name.trim().is_empty() {
                String::new()
            } else {
                message.text.clone()
            };
            let name_galley = ui.painter().layout_job(egui::text::LayoutJob::simple(
                name,
                egui::FontId::proportional(14.0),
                palette.text,
                text_width,
            ));
            let mut meta = crate::model::Model::file_size(document.size);
            if document.pages > 0 {
                meta.push_str(&format!(" · {} pages", document.pages));
            }
            let meta_galley = ui.painter().layout_no_wrap(
                meta,
                egui::FontId::proportional(11.5),
                palette.secondary,
            );
            let caption_galley = (!caption.trim().is_empty()).then(|| {
                ui.painter().layout_job(egui::text::LayoutJob::simple(
                    caption,
                    egui::FontId::proportional(13.5),
                    palette.text,
                    width - pad * 2.0,
                ))
            });
            let time = ui.painter().layout_no_wrap(
                model.bubble_stamp(message),
                egui::FontId::proportional(11.0),
                palette.secondary,
            );

            let name_height = name_galley.size().y;
            let row = (name_height + meta_galley.size().y + 3.0).max(icon);
            let mut height = pad + row + 4.0 + time.size().y + 4.0;
            if let Some(galley) = &caption_galley {
                height += galley.size().y + 6.0;
            }
            let (rect, hit) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

            let fill = if message.from_me {
                palette.bubble_out
            } else {
                palette.bubble_in
            };
            let radius = egui::CornerRadius {
                nw: 8,
                ne: 8,
                sw: if message.from_me { 8 } else { 2 },
                se: if message.from_me { 2 } else { 8 },
            };
            let painter = ui.painter();
            painter.rect_filled(rect, radius, fill);
            let icon_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() + pad, rect.top() + pad),
                egui::vec2(icon, icon),
            );
            painter.rect_filled(icon_rect, egui::CornerRadius::same(8), palette.elevated);
            draw_icon(ui, icon_rect.shrink(8.0), Icon::Document, palette.text);

            let text_x = rect.left() + pad * 2.0 + icon;
            painter.galley(
                egui::pos2(text_x, rect.top() + pad),
                name_galley,
                palette.text,
            );
            painter.galley(
                egui::pos2(text_x, rect.top() + pad + name_height + 2.0),
                meta_galley,
                palette.secondary,
            );
            let mut y = rect.top() + pad + row + 4.0;
            if let Some(galley) = caption_galley {
                let caption_height = galley.size().y;
                painter.galley(egui::pos2(rect.left() + pad, y), galley, palette.text);
                y += caption_height;
            }
            let _ = y;
            painter.galley(
                egui::pos2(
                    rect.right() - pad - time.size().x,
                    rect.bottom() - 4.0 - time.size().y,
                ),
                time,
                palette.secondary,
            );

            if scroll {
                hit.scroll_to_me(Some(egui::Align::Center));
            }
            context_menu(&hit, palette, model, message, &mut action);
            pills_row(ui, model, message);
            ui.add_space(4.0);
            return;
        }

        // A voice note: the official player bubble (play/pause, waveform,
        // elapsed/duration). Playback lives in `audio.rs`.
        if message.kind == "audio"
            && let Some(audio) = message.audio.as_ref()
        {
            let width = max_width.min(300.0);
            let height = 58.0;
            let (rect, hit) =
                ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
            let fill = if message.from_me {
                palette.bubble_out
            } else {
                palette.bubble_in
            };
            let radius = egui::CornerRadius {
                nw: 8,
                ne: 8,
                sw: if message.from_me { 8 } else { 2 },
                se: if message.from_me { 2 } else { 8 },
            };
            let painter = ui.painter();
            painter.rect_filled(rect, radius, fill);
            let mine = playback.id.as_deref() == Some(message.id.as_str());
            let playing = mine && playback.playing;
            let elapsed = if mine { playback.elapsed } else { 0.0 };
            let duration = audio.seconds as f32;
            let ratio = if duration > 0.0 {
                (elapsed / duration).clamp(0.0, 1.0)
            } else {
                0.0
            };
            // Play / pause disc.
            let disc = egui::Rect::from_center_size(
                egui::pos2(rect.left() + 30.0, rect.center().y),
                egui::vec2(34.0, 34.0),
            );
            painter.circle_filled(disc.center(), 17.0, palette.accent);
            draw_icon(
                ui,
                disc.shrink(9.0),
                if playing { Icon::Pause } else { Icon::Play },
                egui::Color32::from_rgb(11, 20, 26),
            );
            // Waveform bars (fallback shape when the sender sent none).
            let bars: Vec<u8> = if audio.waveform.is_empty() {
                (0..40)
                    .map(|index| {
                        let seed = message.id.bytes().fold(0u32, |acc, byte| {
                            acc.wrapping_mul(31).wrapping_add(byte as u32)
                        });
                        (((seed >> (index % 8)) & 0x1F) as u8 + 40).min(180)
                    })
                    .collect()
            } else {
                audio.waveform.clone()
            };
            let wave_left = rect.left() + 54.0;
            let wave_right = rect.right() - 10.0;
            let slot = (wave_right - wave_left) / bars.len().max(1) as f32;
            let middle = rect.center().y - 3.0;
            for (index, level) in bars.iter().enumerate() {
                let x = wave_left + slot * index as f32 + slot * 0.25;
                let amplitude = 3.0 + (*level as f32 / 255.0) * 16.0;
                let played = (index as f32 / bars.len().max(1) as f32) <= ratio;
                painter.line_segment(
                    [
                        egui::pos2(x, middle - amplitude),
                        egui::pos2(x, middle + amplitude),
                    ],
                    egui::Stroke::new(
                        (slot * 0.5).clamp(1.0, 2.5),
                        if played {
                            palette.accent
                        } else {
                            palette.secondary
                        },
                    ),
                );
            }
            let label = if mine && elapsed > 0.5 {
                format!("{:.0}s / {}", elapsed, stamp_seconds(duration))
            } else {
                stamp_seconds(duration)
            };
            painter.text(
                egui::pos2(rect.right() - 10.0, rect.bottom() - 8.0),
                egui::Align2::RIGHT_BOTTOM,
                label,
                egui::FontId::proportional(10.5),
                palette.secondary,
            );
            if hit.clicked() {
                action = Some(BubbleAction::PlayAudio(message.id.clone()));
            }
            context_menu(&hit, palette, model, message, &mut action);
            if scroll {
                hit.scroll_to_me(Some(egui::Align::Center));
            }
            ui.add_space(4.0);
            return;
        }

        let body = model.bubble_body(message);
        let fill = if message.from_me {
            palette.bubble_out
        } else {
            palette.bubble_in
        };
        // WhatsApp's tail: the corner on the sender's side is square-ish.
        let radius = egui::CornerRadius {
            nw: 8,
            ne: 8,
            sw: if message.from_me { 8 } else { 2 },
            se: if message.from_me { 2 } else { 8 },
        };
        let padding = 9.0;
        let max_text = max_width - padding * 2.0;
        // Measure the body so the bubble hugs the text (a plain `Frame` lets a
        // right-aligned child stretch it to the full width and height).
        let text_job = match search {
            Some(needle) => {
                let mut job = highlight_job(&body, needle, palette.text);
                job.wrap.max_width = max_text;
                job
            }
            None => egui::text::LayoutJob::simple(
                body.clone(),
                egui::FontId::proportional(14.0),
                palette.text,
                max_text,
            ),
        };
        let text = ui.painter().layout_job(text_job);
        // Emoji are painted as colour pictures (the text font has outlines only,
        // which is why they used to show as boxes); emoji-only messages get the
        // larger cell the official client uses.
        let rich = search.is_none() && has_emoji(&body);
        let rich_body = rich.then(|| {
            let px = if emoji_only(&body) { 44 } else { 19 };
            layout_body(ui, &body, palette.text, max_text, px)
        });
        let body_size = rich_body
            .as_ref()
            .map(|layout| layout.size)
            .unwrap_or_else(|| text.size());
        let time = ui.painter().layout_no_wrap(
            model.bubble_stamp(message),
            egui::FontId::proportional(11.0),
            palette.secondary,
        );
        // Our own bubbles carry the delivery ticks: one grey check once the
        // server has it, two grey when delivered, two blue when read.
        let tick = message
            .from_me
            .then(|| model.tick_status(message).to_string());
        let tick_width = if tick.is_some() { 16.0 } else { 0.0 };
        let sender = message
            .sender_name
            .clone()
            .filter(|_| message.is_group && !message.from_me);
        let sender_galley = sender.as_ref().map(|name| {
            ui.painter().layout_no_wrap(
                name.clone(),
                egui::FontId::proportional(12.5),
                sender_color(name, palette),
            )
        });
        let width =
            (body_size.x.max(time.size().x + tick_width + 10.0) + padding * 2.0).min(max_width);
        let mut height = body_size.y + time.size().y + 8.0;
        if let Some(galley) = &sender_galley {
            height += galley.size().y + 1.0;
        }
        let (rect, hit) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
        let painter = ui.painter();
        painter.rect_filled(rect, radius, fill);
        if search.is_some() {
            painter.rect_stroke(
                rect,
                radius,
                egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(240, 200, 60)),
                egui::StrokeKind::Inside,
            );
        }
        let mut y = rect.top() + 5.0;
        if let Some(galley) = sender_galley {
            let inner = galley.size().y;
            painter.galley(egui::pos2(rect.left() + padding, y), galley, palette.text);
            y += inner + 1.0;
        }
        match &rich_body {
            Some(layout) => paint_body(painter, egui::pos2(rect.left() + padding, y), layout),
            None => {
                painter.galley(egui::pos2(rect.left() + padding, y), text, palette.text);
            }
        }
        painter.galley(
            egui::pos2(
                rect.right() - padding - tick_width - time.size().x,
                rect.bottom() - 4.0 - time.size().y,
            ),
            time,
            palette.secondary,
        );
        if let Some(status) = tick {
            let read = matches!(status.as_str(), "read" | "played");
            let delivered = matches!(status.as_str(), "delivered" | "read" | "played");
            let name = if delivered { "done_all" } else { "check" };
            let color = if read {
                palette.read
            } else {
                palette.secondary
            };
            let tick_rect = egui::Rect::from_min_size(
                egui::pos2(rect.right() - padding - 14.0, rect.bottom() - 17.0),
                egui::vec2(14.0, 14.0),
            );
            if let Some(texture) = crate::icons::texture(ui.ctx(), name, 14, color) {
                painter.image(
                    texture.id(),
                    tick_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
        if scroll {
            hit.scroll_to_me(Some(egui::Align::Center));
        }
        context_menu(&hit, palette, model, message, &mut action);
        pills_row(ui, model, message);
        ui.add_space(4.0);
    });
    action
}

/// WhatsApp's message menu: the quick reaction bar, then the rows `wa-ui`
/// allows for this message.
fn context_menu(
    response: &egui::Response,
    palette: &crate::theme::Palette,
    model: &Model,
    message: &MessageView,
    action: &mut Option<BubbleAction>,
) {
    response.context_menu(|ui| {
        ui.set_min_width(212.0);
        ui.add_space(2.0);
        // The quick reaction bar.
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            for emoji in ["👍", "❤️", "😂", "😮", "😢", "🙏"] {
                let reaction = ui.add(
                    egui::Button::new(egui::RichText::new(emoji).size(19.0))
                        .frame(false)
                        .min_size(egui::vec2(29.0, 29.0)),
                );
                if reaction
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    *action = Some(BubbleAction::React(message.id.clone(), emoji.to_string()));
                    ui.close();
                }
            }
        });
        ui.add_space(2.0);
        let menu = model.message_menu(message);
        if menu.reply && menu_row(ui, palette, Icon::Reply, "Reply") {
            *action = Some(BubbleAction::Reply(message.id.clone()));
            ui.close();
        }
        if menu.copy && menu_row(ui, palette, Icon::Copy, "Copy") {
            ui.ctx().copy_text(message.text.clone());
            ui.close();
        }
        if menu.forward && menu_row(ui, palette, Icon::Forward, "Forward") {
            *action = Some(BubbleAction::Forward(message.id.clone()));
            ui.close();
        }
        if menu.edit && menu_row(ui, palette, Icon::Edit, "Edit") {
            *action = Some(BubbleAction::Edit(message.id.clone()));
            ui.close();
        }
        if menu.star && menu_row(ui, palette, Icon::Star, &menu.star_label) {
            *action = Some(BubbleAction::Star(message.id.clone()));
            ui.close();
        }
        ui.add_space(2.0);
        if menu_row(ui, palette, Icon::Delete, "Delete") {
            *action = Some(BubbleAction::Delete(message.id.clone()));
            ui.close();
        }
        ui.add_space(2.0);
    });
}

/// One row of the message menu: icon, label, and a rounded hover highlight.
fn menu_row(ui: &mut egui::Ui, palette: &crate::theme::Palette, icon: Icon, label: &str) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), palette.hover);
    }
    draw_icon(
        ui,
        egui::Rect::from_min_size(
            egui::pos2(rect.left() + 9.0, rect.center().y - 9.0),
            egui::vec2(18.0, 18.0),
        ),
        icon,
        palette.secondary,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 35.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        palette.text,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// The reaction pills under a message, ours in blue like WhatsApp.
fn pills_row(ui: &mut egui::Ui, model: &Model, message: &MessageView) {
    let pills = model.reactions_for(&message.id);
    if pills.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        for (emoji, count, mine) in pills {
            let label = if count > 1 {
                format!("{emoji} {count}")
            } else {
                emoji
            };
            let mut text = egui::RichText::new(label).size(13.0);
            if mine {
                text = text.color(egui::Color32::from_rgb(83, 189, 235));
            }
            ui.label(text);
        }
    });
}

/// Decode a media file into a texture, once. Absent until the file exists —
/// and absent when it is not a readable image.
fn texture_at(
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<String, Option<egui::TextureHandle>>,
    key: &str,
    path: Option<String>,
) -> Option<egui::TextureHandle> {
    if let Some(texture) = cache.get(key) {
        return texture.clone();
    }
    let bytes = std::fs::read(path?).ok()?;
    let mut image = image::load_from_memory(&bytes).ok()?;
    // Big screenshots are common; cap the upload size so decoding and texture
    // creation stay cheap (the window never paints wider than this).
    if image.width() > 720 {
        let scale = 720.0 / image.width() as f32;
        image = image.resize(
            (image.width() as f32 * scale) as u32,
            (image.height() as f32 * scale) as u32,
            image::imageops::FilterType::Triangle,
        );
    }
    let image = image.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    let texture = ctx.load_texture(key.to_string(), color, egui::TextureOptions::LINEAR);
    cache.insert(key.to_string(), Some(texture.clone()));
    Some(texture)
}

/// A laid-out label with every occurrence of the query on a yellow band.
fn highlight_job(text: &str, needle: &str, color: egui::Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let plain = egui::TextFormat {
        color,
        ..Default::default()
    };
    let marked = egui::TextFormat {
        color: egui::Color32::from_rgb(24, 24, 24),
        background: egui::Color32::from_rgb(240, 200, 60),
        ..Default::default()
    };
    let needle: Vec<char> = needle.trim().to_lowercase().chars().collect();
    let chars: Vec<char> = text.chars().collect();
    if needle.is_empty() || chars.is_empty() {
        job.append(text, 0.0, plain);
        return job;
    }
    // Lowercase one char at a time so indices stay aligned with `chars`.
    let lowered: Vec<char> = chars
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let mut index = 0;
    let mut start = 0;
    while index + needle.len() <= lowered.len() {
        if lowered[index..index + needle.len()] == needle[..] {
            if start < index {
                job.append(
                    &chars[start..index].iter().collect::<String>(),
                    0.0,
                    plain.clone(),
                );
            }
            job.append(
                &chars[index..index + needle.len()]
                    .iter()
                    .collect::<String>(),
                0.0,
                marked.clone(),
            );
            index += needle.len();
            start = index;
        } else {
            index += 1;
        }
    }
    if start < chars.len() {
        job.append(&chars[start..].iter().collect::<String>(), 0.0, plain);
    }
    job
}

impl WaDesk {
    /// The login screen: the QR (or pairing code), the hint line and the
    /// actions that get a fresh code.
    fn gate(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading("WA Desk");
                ui.label(egui::RichText::new(self.model.login.hint()).weak());

                match self.model.login.clone() {
                    LoginState::Qr(svg) => {
                        if let Some(texture) = self.qr_texture(ctx, &svg) {
                            let side = 280.0;
                            ui.add_space(12.0);
                            ui.image((texture.id(), egui::vec2(side, side)));
                        } else {
                            ui.label(
                                egui::RichText::new("could not draw the QR code")
                                    .color(self.palette.danger),
                            );
                        }
                    }
                    LoginState::PairCode(code) => {
                        ui.add_space(16.0);
                        ui.label(egui::RichText::new(code).size(30.0).monospace());
                        ui.label(
                            egui::RichText::new("Linked devices → Link with phone number")
                                .small()
                                .weak(),
                        );
                    }
                    LoginState::Expired | LoginState::Waiting => {}
                    LoginState::Linked => {}
                }

                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    if crate::login::can_restart(&self.model.login)
                        && ui.button("Request a new code").clicked()
                    {
                        self.model.login_restarted();
                        self.send(Command::Restart);
                    }
                });

                ui.add_space(24.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Or link with a phone number").small());
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 90.0).max(120.0);
                    ui.add_sized(
                        [width, 28.0],
                        egui::TextEdit::singleline(&mut self.model.pair_phone)
                            .hint_text("628123456789"),
                    );
                    if ui
                        .add_sized([80.0, 28.0], egui::Button::new("Get code"))
                        .clicked()
                    {
                        let phone: String = self
                            .model
                            .pair_phone
                            .chars()
                            .filter(|c| c.is_ascii_digit())
                            .collect();
                        if !phone.is_empty() {
                            self.send(Command::PairCode { phone });
                        }
                    }
                });
                ui.add_space(10.0);
                ui.label(egui::RichText::new(&self.model.status).small().weak());
            });
        });
    }

    /// Rasterise the QR SVG once per code.
    fn qr_texture(&mut self, ctx: &egui::Context, svg: &str) -> Option<egui::TextureHandle> {
        if let Some((_, texture)) = self.qr.as_ref().filter(|(cached, _)| cached == svg) {
            return Some(texture.clone());
        }
        let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).ok()?;
        let size = tree.size().to_int_size();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width().max(1), size.height().max(1))?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::default(),
            &mut pixmap.as_mut(),
        );
        let image = egui::ColorImage::from_rgba_premultiplied(
            [size.width() as usize, size.height() as usize],
            pixmap.data(),
        );
        let texture = ctx.load_texture("login-qr", image, egui::TextureOptions::NEAREST);
        self.qr = Some((svg.to_string(), texture.clone()));
        Some(texture)
    }
}

impl WaDesk {
    /// Status: the poster rail, or the story viewer when one is open.
    fn status_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.model.viewer.is_some() {
                self.status_viewer(ui, ctx);
                return;
            }
            ui.horizontal(|ui| {
                ui.heading("Status");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Back to chats").clicked() {
                        self.show_status = false;
                    }
                });
            });
            ui.separator();

            let groups = self.model.status_groups();
            if groups.is_empty() {
                ui.add_space(20.0);
                ui.label(egui::RichText::new("No status updates").weak());
                return;
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for group in &groups {
                        let ring = if group.unseen {
                            egui::Color32::from_rgb(37, 211, 102)
                        } else {
                            egui::Color32::from_gray(120)
                        };
                        ui.horizontal(|ui| {
                            // The ring: unseen green, seen grey.
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            ui.painter().circle_stroke(
                                rect.center(),
                                7.0,
                                egui::Stroke::new(2.5_f32, ring),
                            );
                            let label = format!("{}  ·  {} update(s)", group.name, group.count);
                            if ui.button(label).clicked() {
                                let jid = group.jid.clone();
                                self.model.open_status(&jid);
                            }
                        });
                    }
                });
        });
    }

    /// One story frame: progress segments, the body, and prev/next/close.
    fn status_viewer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(update) = self.model.current_status().cloned() else {
            self.model.close_status();
            return;
        };
        let Some(chrome) = self.model.status_chrome() else {
            self.model.close_status();
            return;
        };

        // Segments: everything before the frame is full, the active one fills.
        let (group, frame) = self.model.viewer.unwrap_or((0, 0));
        let groups = self.model.status_groups();
        let frames = groups.get(group).map(|row| row.count).unwrap_or(1);
        let ratio = self.model.status_progress() as f32;
        ui.horizontal(|ui| {
            for index in 0..frames {
                let fill = if index < frame {
                    1.0
                } else if index == frame {
                    ratio.clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let width = (ui.available_width() - (frames as f32) * 4.0) / frames.max(1) as f32;
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(width, 4.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, 2.0, egui::Color32::from_gray(70));
                let mut done = rect;
                done.set_width(rect.width() * fill);
                ui.painter()
                    .rect_filled(done, 2.0, egui::Color32::from_gray(220));
            }
        });
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&update.name).strong());
            ui.label(
                egui::RichText::new(self.model.bubble_stamp(&status_as_message(&update)))
                    .small()
                    .weak(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✕").clicked() {
                    self.model.close_status();
                }
            });
        });

        let bg = update.background.trim_start_matches('#');
        let color = u32::from_str_radix(bg, 16)
            .map(|value| {
                egui::Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
            })
            .unwrap_or(egui::Color32::from_rgb(36, 38, 38));
        egui::Frame::new()
            .fill(color)
            .inner_margin(egui::Margin::same(16))
            .corner_radius(egui::CornerRadius::same(10))
            .show(ui, |ui| {
                ui.set_min_height(240.0);
                ui.centered_and_justified(|ui| {
                    let body = if update.text.trim().is_empty() {
                        wa_ui::chat::media_placeholder(&update.kind).to_string()
                    } else {
                        update.text.clone()
                    };
                    ui.label(
                        egui::RichText::new(body)
                            .size(22.0)
                            .color(egui::Color32::WHITE),
                    );
                });
            });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if !chrome.prev_hidden && ui.button("◀").clicked() {
                self.model.step_status(false);
            }
            if !chrome.next_hidden && ui.button("▶").clicked() {
                self.model.step_status(true);
            }
            if chrome.own_story {
                ui.label(
                    egui::RichText::new(format!("{} view(s)", chrome.view_count_label))
                        .small()
                        .weak(),
                );
            }
        });

        // A frame that runs out advances on its own, like the phone.
        if ratio >= 1.0 {
            self.model.step_status(true);
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

/// A story frame as a message, so `wa-ui`'s clock label can format it.
fn status_as_message(update: &wa_ui::status::StatusUpdateView) -> MessageView {
    MessageView {
        chat: update.sender.clone(),
        chat_name: update.name.clone(),
        sender: Some(update.sender.clone()),
        sender_name: Some(update.name.clone()),
        sender_phone: None,
        id: update.id.clone(),
        from_me: update.from_me,
        is_group: false,
        timestamp: update.timestamp,
        kind: update.kind.clone(),
        text: update.text.clone(),
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

/// Fallback faces the bundled set lacks, in fallback order: emoji, Chinese
/// (simplified *and* traditional), Japanese, then Korean. Before this, non-Latin
/// text drew as empty boxes.
fn install_fonts(ctx: &egui::Context) {
    let groups: [(&str, &[&str]); 4] = [
        (
            "system-emoji",
            &[
                // Windows: Segoe UI Emoji (outlines are readable even without colour).
                "C:\\Windows\\Fonts\\seguiemj.ttf",
                "C:\\Windows\\Fonts\\seguisym.ttf",
                // Linux.
                "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
                "/usr/share/fonts/noto/NotoColorEmoji.ttf",
                "/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf",
            ],
        ),
        (
            "system-cjk",
            &[
                // Microsoft YaHei (GB18030: simplified, traditional, kana), then
                // Traditional-first faces, then the older SimSun.
                "C:\\Windows\\Fonts\\msyh.ttc",
                "C:\\Windows\\Fonts\\msyh.ttf",
                "C:\\Windows\\Fonts\\msjh.ttc",
                "C:\\Windows\\Fonts\\simsun.ttc",
                "C:\\Windows\\Fonts\\mingliu.ttc",
                // Linux / macOS.
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
                "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
                "/usr/share/fonts/truetype/arphic/uming.ttc",
                "/System/Library/Fonts/PingFang.ttc",
            ],
        ),
        (
            "system-japanese",
            &[
                // Kana and Japanese-only kanji the Chinese faces do not carry.
                "C:\\Windows\\Fonts\\YuGothM.ttc",
                "C:\\Windows\\Fonts\\YuGothR.ttc",
                "C:\\Windows\\Fonts\\meiryo.ttc",
                "C:\\Windows\\Fonts\\msgothic.ttc",
                "/usr/share/fonts/truetype/takao-gothic/TakaoPGothic.ttf",
                "/usr/share/fonts/truetype/vlgothic/VL-Gothic-Regular.ttf",
            ],
        ),
        (
            "system-hangul",
            &[
                // Hangul is not in the Chinese faces above.
                "C:\\Windows\\Fonts\\malgun.ttf",
                "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
            ],
        ),
    ];
    let mut fonts = egui::FontDefinitions::default();
    let mut added = false;
    for (key, candidates) in groups {
        let Some(bytes) = candidates.iter().find_map(|path| std::fs::read(path).ok()) else {
            continue;
        };
        diag(&format!("font {key}: loaded"));
        fonts.font_data.insert(
            key.to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push(key.to_owned());
        }
        added = true;
    }
    if added {
        ctx.set_fonts(fonts);
    }
}

/// What the viewer knows about a video message.
#[derive(Debug, Clone)]
pub enum VideoState {
    Preparing,
    Ready(crate::video::Video),
    Failed(String),
}

/// Append a line to `diagnostics.log` when `WA_DESK_DIAG=1` (the windowing
/// backend detaches the console, so tracing is not readable from a GUI run).
pub fn diag(message: &str) {
    if std::env::var("WA_DESK_DIAG").is_err() {
        return;
    }
    let dir = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("io.github.dirttw.wadesk-native");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("diagnostics.log"))
    {
        use std::io::Write as _;
        let _ = writeln!(file, "{message}");
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `m:ss` for a clip length.
fn stamp_seconds(seconds: f32) -> String {
    let total = seconds.max(0.0) as i64;
    format!("{}:{:02}", total / 60, total % 60)
}

impl WaDesk {
    /// Channels: followed newsletters and their read-only posts.
    fn channels_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Channels");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Back to chats").clicked() {
                        self.show_channels = false;
                    }
                });
            });
            ui.separator();

            let rows = self.model.channel_rows();
            if rows.is_empty() {
                ui.add_space(20.0);
                ui.label(egui::RichText::new("No followed channels").weak());
                return;
            }

            ui.horizontal_top(|ui| {
                // The channel rail.
                ui.vertical(|ui| {
                    ui.set_width(220.0);
                    for (row, followers) in &rows {
                        let selected = self.model.active.as_deref() == Some(row.jid.as_str());
                        let label = format!("{}\n{}", row.name, followers);
                        if ui
                            .add_sized(
                                [ui.available_width(), 44.0],
                                egui::Button::new(label).selected(selected),
                            )
                            .clicked()
                        {
                            self.model.active = Some(row.jid.clone());
                        }
                    }
                });
                ui.separator();

                // Posts, read-only: no composer for a channel.
                ui.vertical(|ui| {
                    let Some(jid) = self.model.active.clone() else {
                        ui.label(egui::RichText::new("Pick a channel").weak());
                        return;
                    };
                    let posts = self.model.channel_posts(&jid);
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for post in &posts {
                                let header = format!(
                                    "{}  ·  {}",
                                    self.model.title(
                                        &self.model.chats.get(&jid).cloned().unwrap_or_default()
                                    ),
                                    self.model.bubble_stamp(post)
                                );
                                ui.label(egui::RichText::new(header).small().weak());
                                let body = self.model.bubble_body(post);
                                egui::Frame::new()
                                    .fill(egui::Color32::from_rgb(36, 38, 38))
                                    .inner_margin(egui::Margin::symmetric(10, 7))
                                    .corner_radius(egui::CornerRadius::same(8))
                                    .show(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new(body)
                                                .color(egui::Color32::from_gray(240)),
                                        );
                                    });
                                ui.add_space(6.0);
                            }
                        });
                    ui.separator();
                    ui.label(
                        egui::RichText::new(
                            "Read-only: following a channel does not allow posting",
                        )
                        .small()
                        .weak(),
                    );
                });
            });
        });
    }
}

impl WaDesk {
    /// Settings: the official two-pane screen - the section list on the left,
    /// the selected page as cards on the right.
    fn settings_panel(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    self.settings_sections(ui, &palette);
                    ui.add_space(2.0);
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.horizontal_top(|ui| {
                                ui.add_space(28.0);
                                ui.vertical(|ui| {
                                    let width = (ui.available_width() - 40.0).clamp(240.0, 640.0);
                                    ui.set_width(width);
                                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                    ui.add_space(24.0);
                                    match self.settings_section {
                                        0 => self.general_detail(ui, &palette),
                                        1 => self.chats_detail(ui, &palette),
                                        2 => self.ai_detail(ui, &palette),
                                        3 => self.updates_detail(ui, &palette),
                                        _ => self.help_detail(ui, &palette),
                                    }
                                    if !self.settings_note.is_empty() {
                                        ui.add_space(12.0);
                                        ui.label(
                                            egui::RichText::new(&self.settings_note)
                                                .size(12.5)
                                                .color(palette.secondary),
                                        );
                                    }
                                    ui.add_space(28.0);
                                });
                            });
                        });
                });
            });
    }

    /// The left column: account header, the section rows and Log out.
    fn settings_sections(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        ui.vertical(|ui| {
            let width = (ui.available_width() * 0.42).clamp(230.0, 330.0);
            ui.set_width(width);
            ui.add_space(20.0);
            let me = self.model.me_jid.clone();
            self.ensure_avatar(ui.ctx(), &me);
            let avatar = self.avatar_for(&me).cloned();
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                paint_avatar(ui, &self.model.own_name(), 56.0, avatar.as_ref());
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(self.model.own_name())
                            .size(17.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.label(
                        egui::RichText::new(&self.model.status)
                            .size(12.5)
                            .color(palette.secondary),
                    );
                });
            });
            ui.add_space(20.0);
            let sections = [
                (Icon::Settings, "General", "Startup, updates, about"),
                (Icon::Chats, "Chats", "Theme, history, sync"),
                (Icon::Emoji, "AI assistant", "Auto-reply with your key"),
                (Icon::Document, "Updates", "Version and releases"),
                (
                    Icon::Groups,
                    "Help and feedback",
                    "Project, changelog, issues",
                ),
            ];
            for (index, (icon, title, subtitle)) in sections.iter().enumerate() {
                let selected = self.settings_section == index;
                let response = settings_list_row(
                    ui,
                    palette,
                    *icon,
                    title,
                    Some(subtitle),
                    selected,
                    palette.secondary,
                );
                if response.clicked() {
                    self.settings_section = index;
                }
            }
            ui.add_space(6.0);
            let response = settings_list_row(
                ui,
                palette,
                Icon::Logout,
                "Log out",
                None,
                false,
                palette.danger,
            );
            if response.clicked() {
                self.send(Command::Relink);
                self.settings_note =
                    "Unlinked - scan the QR with the phone to link again.".to_string();
            }
        });
    }

    /// The page title and one-line description at the top of the detail pane.
    fn settings_header(
        ui: &mut egui::Ui,
        palette: &crate::theme::Palette,
        title: &str,
        subtitle: &str,
    ) {
        ui.label(
            egui::RichText::new(title)
                .size(22.0)
                .strong()
                .color(palette.text),
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(subtitle)
                .size(13.0)
                .color(palette.secondary),
        );
    }

    /// General: start-up behaviour and version.
    fn general_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "General",
            "Startup behaviour and this installation.",
        );
        let mut launch = crate::autostart::enabled();
        let mut toggle = false;
        settings_card(ui, palette, "STARTUP AND CLOSE", |ui| {
            settings_row(
                ui,
                palette,
                "Launch WA Desk at login",
                "Starts with Windows through your account's Run key; nothing is written outside it.",
                |ui| {
                    toggle = settings_switch(ui, palette, launch).clicked();
                },
            );
        });
        if toggle {
            launch = !launch;
            self.settings_note = match crate::autostart::set(launch) {
                Ok(()) => {
                    if launch {
                        "WA Desk will start with Windows.".to_string()
                    } else {
                        "WA Desk will no longer start with Windows.".to_string()
                    }
                }
                Err(err) => err,
            };
        }

        settings_card(ui, palette, "ABOUT", |ui| {
            settings_row(
                ui,
                palette,
                "Version",
                "Unofficial WhatsApp client in pure Rust.",
                |ui| {
                    ui.label(
                        egui::RichText::new(env!("CARGO_PKG_VERSION"))
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Storage",
                "Chat cache, media and settings live here:",
                |_ui| {},
            );
            ui.label(
                egui::RichText::new("%APPDATA%\\io.github.dirttw.wadesk-native")
                    .size(12.0)
                    .monospace()
                    .color(palette.secondary),
            );
        });
    }

    /// Chats: theme, chat history and the sync actions.
    fn chats_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Chats",
            "Appearance and what the phone sends us.",
        );
        let system_dark = ui.ctx().style().visuals.dark_mode;
        let current = self.settings.theme.clone();
        let mut chosen: Option<String> = None;
        settings_card(ui, palette, "THEME", |ui| {
            for choice in ["light", "dark", "system"] {
                if settings_radio_row(
                    ui,
                    palette,
                    wa_ui::settings::theme_label(choice),
                    current == choice,
                )
                .clicked()
                {
                    chosen = Some(choice.to_string());
                }
            }
        });
        if let Some(choice) = chosen {
            self.settings.theme = choice;
            let _ = self.settings.save(&self.settings_path);
        }
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "{} · showing {}",
                self.settings.theme_label(),
                self.settings.resolved_theme(system_dark)
            ))
            .size(12.0)
            .color(palette.secondary),
        );

        let mut notifications = self.settings.notifications;
        let mut toggled = false;
        settings_card(ui, palette, "NOTIFICATIONS", |ui| {
            settings_row(
                ui,
                palette,
                "Desktop notifications",
                "A toast when a message arrives while WA Desk is in the background.",
                |ui| {
                    if settings_switch(ui, palette, notifications).clicked() {
                        notifications = !notifications;
                        toggled = true;
                    }
                },
            );
        });
        if toggled {
            self.settings.notifications = notifications;
            let _ = self.settings.save(&self.settings_path);
            self.settings_note = if notifications {
                "Desktop notifications are on.".to_string()
            } else {
                "Desktop notifications are off.".to_string()
            };
        }

        let mut action: Option<&'static str> = None;
        settings_card(ui, palette, "CHAT HISTORY", |ui| {
            ui.label(
                egui::RichText::new(
                    "Phones only push the full chat list to a newly linked device, so this asks \
                     the linked session for one batch at a time (eight chats).",
                )
                .size(12.5)
                .color(palette.secondary),
            );
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 22.0;
                for (label, key) in [
                    ("Sync chat history", "history"),
                    ("Sync more chats", "more"),
                    ("Sync groups", "groups"),
                    ("Refresh statuses", "status"),
                ] {
                    if settings_link(ui, palette, label) {
                        action = Some(key);
                    }
                }
            });
        });
        if let Some(action) = action {
            match action {
                "history" => {
                    self.send(Command::InitialSync);
                    self.send(Command::SyncChatState);
                    self.settings_note = "Asking the phone for history…".to_string();
                }
                "more" => {
                    self.send(Command::SyncMore);
                    self.settings_note = "Asking for the next chats…".to_string();
                }
                "groups" => {
                    self.send(Command::SyncGroups);
                    self.settings_note = "Asking for participating groups…".to_string();
                }
                _ => {
                    self.send(Command::StatusHistory);
                    self.settings_note = "Asking for recent statuses…".to_string();
                }
            }
        }
    }

    /// AI assistant: the encrypted key and the model configuration.
    fn ai_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "AI assistant",
            "Bring your own OpenAI-compatible endpoint.",
        );
        let hint = self.settings.key_hint();
        settings_card(ui, palette, "BEHAVIOUR", |ui| {
            settings_row(
                ui,
                palette,
                "AI mode",
                "Answer saved contacts, and groups that mention you.",
                |ui| {
                    let mut on = self.settings.ai.enabled;
                    if settings_switch(ui, palette, on).clicked() {
                        on = !on;
                    }
                    self.settings.ai.enabled = on;
                },
            );
            settings_row(
                ui,
                palette,
                "Require /ai trigger",
                "Only reply when a message asks with /ai.",
                |ui| {
                    let mut on = self.settings.ai.require_trigger;
                    if settings_switch(ui, palette, on).clicked() {
                        on = !on;
                    }
                    self.settings.ai.require_trigger = on;
                },
            );
        });

        settings_card(ui, palette, "CONNECTION", |ui| {
            ui.label(
                egui::RichText::new(hint.clone())
                    .size(12.0)
                    .color(palette.secondary),
            );
            ui.add_space(8.0);
            settings_field(
                ui,
                palette,
                "Base URL",
                "https://opencode.ai/zen/go/v1",
                &mut self.settings.ai.base_url,
                false,
            );
            settings_field(
                ui,
                palette,
                "Model",
                "the chat model id",
                &mut self.settings.ai.model,
                false,
            );
            settings_field(
                ui,
                palette,
                "Vision model",
                "optional - the chat model when empty",
                &mut self.settings.ai.vision_model,
                false,
            );
            settings_field(
                ui,
                palette,
                "API key",
                "leave blank to keep the stored key",
                &mut self.settings.ai.api_key,
                true,
            );
        });

        settings_card(ui, palette, "PERSONA", |ui| {
            ui.label(
                egui::RichText::new("System prompt")
                    .size(13.0)
                    .color(palette.text),
            );
            ui.add_space(6.0);
            ui.add(
                egui::TextEdit::multiline(&mut self.settings.ai.system_prompt)
                    .desired_width(f32::INFINITY)
                    .desired_rows(4),
            );
        });

        ui.add_space(14.0);
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("Save")
                        .size(13.5)
                        .color(egui::Color32::WHITE),
                )
                .fill(palette.accent)
                .corner_radius(egui::CornerRadius::same(18))
                .min_size(egui::vec2(96.0, 34.0)),
            )
            .clicked()
        {
            let plan = self.settings.ai_plan();
            if !plan.ok {
                self.settings_note = plan.error.unwrap_or_default();
            } else {
                match self.settings.save(&self.settings_path) {
                    Ok(()) => {
                        if !self.settings.ai.api_key.is_empty() {
                            // The key is encrypted at rest (AES-256-GCM under a
                            // random file key); only the masked hint stays in
                            // settings.json.
                            match crate::ai::keys::save(
                                &crate::settings::NativeSettings::data_dir(),
                                &self.settings.ai.api_key,
                            ) {
                                Ok(()) => self.settings.ai.has_key = true,
                                Err(err) => {
                                    self.settings_note = format!("Could not store the key: {err}");
                                    return;
                                }
                            }
                        }
                        self.settings.ai.api_key.clear();
                        self.settings_note = plan.toast;
                    }
                    Err(err) => self.settings_note = err,
                }
            }
        }
    }

    /// Updates: the GitHub pre-release channel.
    fn updates_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(ui, palette, "Updates", "Version and GitHub releases.");
        let version = env!("CARGO_PKG_VERSION");
        let state = self.update.clone();
        let busy = matches!(state, UpdateState::Checking | UpdateState::Downloading(_));
        settings_card(ui, palette, "THIS INSTALL", |ui| {
            settings_row(
                ui,
                palette,
                "Installed version",
                "Every WA Desk release is a pre-release.",
                |ui| {
                    ui.label(
                        egui::RichText::new(format!("v{version}"))
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(ui, palette, "Channel", "dirtTW/wa-desk", |ui| {
                if !busy && settings_link(ui, palette, "Check now") {
                    self.check_updates();
                }
            });
        });

        settings_card(ui, palette, "STATUS", |ui| match state {
            UpdateState::Idle => {
                ui.label(
                    egui::RichText::new("Not checked yet.")
                        .size(13.0)
                        .color(palette.secondary),
                );
            }
            UpdateState::Checking => {
                ui.label(
                    egui::RichText::new("Checking the release list…")
                        .size(13.0)
                        .color(palette.secondary),
                );
            }
            UpdateState::UpToDate => {
                ui.label(
                    egui::RichText::new("You are on the newest release.")
                        .size(13.0)
                        .color(palette.secondary),
                );
            }
            UpdateState::Available(release) => {
                let megabytes = release.size as f64 / 1_048_576.0;
                ui.label(
                    egui::RichText::new(format!(
                        "{} is available ({megabytes:.1} MB)",
                        release.tag
                    ))
                    .size(13.0)
                    .color(palette.text),
                );
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 20.0;
                    if settings_link(ui, palette, "Download & install") {
                        self.download_update(release.clone());
                    }
                    if !release.notes_url.is_empty() && settings_link(ui, palette, "Release notes")
                    {
                        ui.ctx()
                            .open_url(egui::OpenUrl::new_tab(&release.notes_url));
                    }
                });
            }
            UpdateState::Downloading(release) => {
                ui.label(
                    egui::RichText::new(format!("Downloading {}…", release.tag))
                        .size(13.0)
                        .color(palette.secondary),
                );
            }
            UpdateState::Ready(path) => {
                ui.label(
                    egui::RichText::new("Downloaded - restarting finishes the update.")
                        .size(13.0)
                        .color(palette.secondary),
                );
                ui.add_space(8.0);
                if settings_link(ui, palette, "Restart now") {
                    match updates::install(&path) {
                        Ok(()) => std::process::exit(0),
                        Err(err) => self.update = UpdateState::Failed(err),
                    }
                }
            }
            UpdateState::Failed(err) => {
                ui.label(egui::RichText::new(err).size(13.0).color(palette.danger));
            }
        });
    }

    /// Help and feedback: the project links.
    fn help_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Help and feedback",
            "WA Desk is unofficial and open.",
        );
        settings_card(ui, palette, "PROJECT", |ui| {
            for (label, url) in [
                ("Source code", "https://github.com/dirtTW/wa-desk"),
                (
                    "Change log",
                    "https://github.com/dirtTW/wa-desk/blob/main/CHANGELOG.md",
                ),
                (
                    "Report an issue",
                    "https://github.com/dirtTW/wa-desk/issues",
                ),
            ] {
                if settings_link(ui, palette, label) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                }
                ui.add_space(8.0);
            }
        });
        settings_card(ui, palette, "ABOUT", |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "WA Desk v{} · MIT licensed · not affiliated with WhatsApp or Meta.",
                    env!("CARGO_PKG_VERSION")
                ))
                .size(12.5)
                .color(palette.secondary),
            );
        });
    }
}

/// One category tab of the emoji picker: the group's own emoji on a pill, with
/// the accent underline the official panel uses for the active tab.
fn emoji_category(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    emoji: &str,
    texture: Option<egui::TextureHandle>,
    selected: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(34.0, 30.0), egui::Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(8),
            if selected {
                palette.selected
            } else {
                palette.hover
            },
        );
    }
    let center = egui::pos2(rect.center().x, rect.center().y);
    match texture {
        Some(texture) => {
            let size = texture.size_vec2();
            let scale = (20.0 / size.x.max(1.0)).min(20.0 / size.y.max(1.0));
            ui.painter().image(
                texture.id(),
                egui::Rect::from_center_size(center, size * scale),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        None => {
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                emoji,
                egui::FontId::proportional(16.0),
                palette.text,
            );
        }
    }
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + 7.0, rect.bottom() - 2.0),
                egui::vec2(rect.width() - 14.0, 2.0),
            ),
            egui::CornerRadius::same(1),
            palette.accent,
        );
    }
    response
}

/// What a notification says: the text, or the media type when it has none.
fn notification_body(kind: &str, text: &str) -> String {
    if !text.trim().is_empty() {
        return text.trim().to_string();
    }
    match kind {
        "image" => "Photo",
        "video" => "Video",
        "audio" => "Voice message",
        "sticker" => "Sticker",
        "document" => "Document",
        _ => "New message",
    }
    .to_string()
}

/// One laid-out piece of a message body: text or a colour emoji.
enum Inline {
    Text(std::sync::Arc<egui::Galley>),
    Emoji(egui::TextureHandle),
}

/// A body laid out with text galleys and colour emoji side by side.
struct BodyLayout {
    placed: Vec<(egui::Pos2, Inline)>,
    size: egui::Vec2,
    emoji_px: f32,
}

/// Is this a character the official client draws as an emoji picture?
fn is_emoji_char(ch: char) -> bool {
    let code = ch as u32;
    code >= 0x1F000
        || (0x2600..=0x27BF).contains(&code)
        || (0x2B00..=0x2BFF).contains(&code)
        || code == 0x00A9
        || code == 0x00AE
}

fn has_emoji(text: &str) -> bool {
    text.chars().any(is_emoji_char)
}

/// WhatsApp paints emoji-only messages much larger.
fn emoji_only(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= 12
        && trimmed.chars().all(|ch| {
            is_emoji_char(ch) || ch == '\u{FE0F}' || ch == '\u{200D}' || ch.is_whitespace()
        })
}

/// Lay out a message body where emoji are colour textures and the rest is text.
/// Words are wrapped at `max_width`; emoji occupy `emoji_px` square cells.
fn layout_body(
    ui: &egui::Ui,
    text: &str,
    color: egui::Color32,
    max_width: f32,
    emoji_px: u32,
) -> BodyLayout {
    enum Token {
        Word(String),
        Emoji(char),
    }

    let font = egui::FontId::proportional(14.0);
    let row_height = ui.fonts(|fonts| fonts.row_height(&font));
    let emoji_size = emoji_px as f32;

    // Split into runs of emoji and text, then emoji cells and words.
    let mut runs: Vec<(String, bool)> = Vec::new();
    for ch in text.chars() {
        let emoji = is_emoji_char(ch) || ch == '\u{FE0F}' || ch == '\u{200D}';
        match runs.last_mut() {
            Some((buffer, kind)) if *kind == emoji => buffer.push(ch),
            _ => runs.push((ch.to_string(), emoji)),
        }
    }
    let mut tokens: Vec<Token> = Vec::new();
    for (run, emoji) in runs {
        if emoji {
            for ch in run.chars() {
                if ch != '\u{FE0F}' && ch != '\u{200D}' {
                    tokens.push(Token::Emoji(ch));
                }
            }
        } else {
            for word in run.split_inclusive(char::is_whitespace) {
                tokens.push(Token::Word(word.to_string()));
            }
        }
    }

    let mut placed = Vec::new();
    let mut x = 0.0f32;
    let mut y = 0.0f32;
    let mut widest = 0.0f32;
    for token in tokens {
        match token {
            Token::Emoji(ch) => {
                if x > 0.0 && x + emoji_size > max_width {
                    x = 0.0;
                    y += row_height;
                }
                let offset = (row_height - emoji_size) * 0.5;
                match crate::emoji_glyph::texture_shared(ui.ctx(), ch, emoji_px) {
                    Some(texture) => {
                        placed.push((egui::pos2(x, y + offset), Inline::Emoji(texture)));
                    }
                    None => {
                        let galley =
                            ui.painter()
                                .layout_no_wrap(ch.to_string(), font.clone(), color);
                        placed.push((egui::pos2(x, y + offset), Inline::Text(galley)));
                    }
                }
                x += emoji_size;
                widest = widest.max(x);
            }
            Token::Word(word) => {
                let galley = ui
                    .painter()
                    .layout_no_wrap(word.clone(), font.clone(), color);
                let width = galley.size().x;
                if x > 0.0 && x + width > max_width {
                    x = 0.0;
                    y += row_height;
                }
                let offset = (row_height - galley.size().y) * 0.5;
                placed.push((egui::pos2(x, y + offset), Inline::Text(galley)));
                x += width;
                widest = widest.max(x);
            }
        }
    }
    BodyLayout {
        placed,
        size: egui::vec2(widest, y + row_height),
        emoji_px: emoji_size,
    }
}

/// Paint a laid-out body at `origin`.
fn paint_body(painter: &egui::Painter, origin: egui::Pos2, body: &BodyLayout) {
    for (pos, inline) in &body.placed {
        let at = origin + pos.to_vec2();
        match inline {
            Inline::Text(galley) => {
                painter.galley(at, galley.clone(), egui::Color32::PLACEHOLDER);
            }
            Inline::Emoji(texture) => {
                let size = texture.size_vec2();
                let scale = (body.emoji_px / size.y.max(1.0)).min(body.emoji_px / size.x.max(1.0));
                painter.image(
                    texture.id(),
                    egui::Rect::from_min_size(at, size * scale),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
        }
    }
}

/// The initials for an avatar fallback ("Ada Lovelace" -> "AL").
fn call_initials(name: &str) -> String {
    let mut letters = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect::<String>();
    if letters.is_empty() {
        letters = name.chars().take(1).collect();
    }
    letters.to_uppercase()
}

/// One tab of the sticker picker: label with the accent underline the official
/// panel uses for the active tab.
fn sticker_tab(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    label: &str,
    selected: bool,
) -> egui::Response {
    let width = ui
        .painter()
        .layout_no_wrap(
            label.to_string(),
            egui::FontId::proportional(13.0),
            palette.text,
        )
        .size()
        .x
        + 18.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(8),
            if selected {
                palette.selected
            } else {
                palette.hover
            },
        );
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(13.0),
        if selected {
            palette.text
        } else {
            palette.secondary
        },
    );
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left() + 8.0, rect.bottom() - 2.0),
                egui::vec2(rect.width() - 16.0, 2.0),
            ),
            egui::CornerRadius::same(1),
            palette.accent,
        );
    }
    response
}

/// One row of the settings section list.
fn settings_list_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    icon: Icon,
    title: &str,
    subtitle: Option<&str>,
    selected: bool,
    color: egui::Color32,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let height = if subtitle.is_some() { 54.0 } else { 44.0 };
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width() - 10.0, height),
            egui::Sense::click(),
        );
        if selected || response.hovered() {
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(10),
                if selected {
                    palette.selected
                } else {
                    palette.hover
                },
            );
        }
        draw_icon(
            ui,
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + 24.0, rect.center().y),
                egui::vec2(20.0, 20.0),
            ),
            icon,
            if selected { palette.accent } else { color },
        );
        let text_left = rect.left() + 46.0;
        match subtitle {
            Some(subtitle) => {
                ui.painter().text(
                    egui::pos2(text_left, rect.center().y - 9.0),
                    egui::Align2::LEFT_CENTER,
                    title,
                    egui::FontId::proportional(14.0),
                    palette.text,
                );
                ui.painter().text(
                    egui::pos2(text_left, rect.center().y + 10.0),
                    egui::Align2::LEFT_CENTER,
                    subtitle,
                    egui::FontId::proportional(11.5),
                    palette.secondary,
                );
            }
            None => {
                ui.painter().text(
                    egui::pos2(text_left, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    title,
                    egui::FontId::proportional(14.0),
                    color,
                );
            }
        }
        response
    })
    .inner
}

/// A card in the detail pane: raised surface, green uppercase title.
fn settings_card(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    title: &str,
    body: impl FnOnce(&mut egui::Ui),
) {
    ui.add_space(16.0);
    egui::Frame::new()
        .fill(palette.elevated)
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(18, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(
                egui::RichText::new(title)
                    .size(11.5)
                    .strong()
                    .color(palette.accent),
            );
            ui.add_space(10.0);
            body(ui);
        });
}

/// A label + description row with an optional trailing control.
fn settings_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    title: &str,
    description: &str,
    control: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal_top(|ui| {
        // Two fixed columns: egui would otherwise wrap the control onto the
        // next line once the wrapping label had taken the whole width. Both
        // columns scale with the pane, so nothing is clipped when it is narrow.
        let width = ui.available_width();
        let control_width = (width * 0.36).clamp(84.0, 168.0);
        let text_width = (width - control_width - 12.0).max(90.0);
        ui.allocate_ui_with_layout(
            egui::vec2(text_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(text_width);
                ui.label(egui::RichText::new(title).size(14.0).color(palette.text));
                if !description.is_empty() {
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(description)
                            .size(12.0)
                            .color(palette.secondary),
                    );
                }
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(control_width, 0.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.set_width(control_width);
                control(ui)
            },
        );
    });
    ui.add_space(10.0);
}

/// A full-width radio row (the theme choices).
fn settings_radio_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    label: &str,
    selected: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(8), palette.hover);
    }
    let center = egui::pos2(rect.right() - 18.0, rect.center().y);
    ui.painter().circle_stroke(
        center,
        8.0,
        egui::Stroke::new(
            1.8_f32,
            if selected {
                palette.accent
            } else {
                palette.secondary
            },
        ),
    );
    if selected {
        ui.painter().circle_filled(center, 4.4, palette.accent);
    }
    ui.painter().text(
        egui::pos2(rect.left() + 2.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        palette.text,
    );
    response
}

/// A Material 3 switch.
fn settings_switch(ui: &mut egui::Ui, palette: &crate::theme::Palette, on: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(40.0, 22.0), egui::Sense::click());
    let track = if on {
        palette.accent
    } else {
        egui::Color32::from_gray(120)
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(11), track);
    let knob_x = if on {
        rect.right() - 11.0
    } else {
        rect.left() + 11.0
    };
    ui.painter().circle_filled(
        egui::pos2(knob_x, rect.center().y),
        8.0,
        egui::Color32::WHITE,
    );
    response
}

/// An accent-coloured text action.
fn settings_link(ui: &mut egui::Ui, palette: &crate::theme::Palette, label: &str) -> bool {
    ui.add(
        egui::Button::new(egui::RichText::new(label).size(13.0).color(palette.accent)).frame(false),
    )
    .clicked()
}

/// A labelled single-line field.
fn settings_field(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    label: &str,
    hint: &str,
    value: &mut String,
    password: bool,
) {
    ui.label(
        egui::RichText::new(label)
            .size(12.5)
            .color(palette.secondary),
    );
    ui.add_space(3.0);
    let mut edit = egui::TextEdit::singleline(value)
        .desired_width(f32::INFINITY)
        .hint_text(hint);
    if password {
        edit = edit.password(true);
    }
    ui.add(edit);
    ui.add_space(10.0);
}

/// Where the native app keeps its own settings.
fn settings_file() -> std::path::PathBuf {
    crate::settings::NativeSettings::data_dir().join("settings.json")
}

impl WaDesk {
    /// Group info: subject, description, member count and the member list
    /// with admin chips.
    fn group_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Group info");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Back").clicked() {
                        self.show_group = false;
                    }
                });
            });
            ui.separator();
            if let Some(err) = self.model.group_error.clone() {
                ui.label(egui::RichText::new(err).color(self.palette.danger));
                return;
            }
            let Some(info) = self.model.group.clone() else {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Loading members…").weak());
                return;
            };
            ui.label(egui::RichText::new(&info.subject).strong());
            ui.label(
                egui::RichText::new(self.model.group_count_label())
                    .small()
                    .weak(),
            );
            if let Some(description) = info
                .description
                .as_ref()
                .filter(|text| !text.trim().is_empty())
            {
                ui.add_space(6.0);
                ui.label(description);
            }
            ui.add_space(10.0);
            ui.separator();
            let chat = self.model.active.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                if ui.button("Add members").clicked() {
                    self.group_add_open = true;
                    self.send(Command::Contacts);
                }
                if ui.button("Leave group").clicked() {
                    self.send(Command::LeaveGroup { chat: chat.clone() });
                    self.show_group = false;
                }
            });
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (name, admin, jid) in self.model.group_members() {
                        ui.horizontal(|ui| {
                            ui.label(name);
                            if admin {
                                ui.label(
                                    egui::RichText::new("Admin")
                                        .small()
                                        .color(egui::Color32::from_rgb(37, 211, 102)),
                                );
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("Remove").clicked() {
                                        self.send(Command::GroupMember {
                                            chat: chat.clone(),
                                            member: jid.clone(),
                                            action: "remove".into(),
                                        });
                                    }
                                    if admin {
                                        if ui.small_button("Demote").clicked() {
                                            self.send(Command::GroupMember {
                                                chat: chat.clone(),
                                                member: jid.clone(),
                                                action: "demote".into(),
                                            });
                                        }
                                    } else if ui.small_button("Promote").clicked() {
                                        self.send(Command::GroupMember {
                                            chat: chat.clone(),
                                            member: jid.clone(),
                                            action: "promote".into(),
                                        });
                                    }
                                },
                            );
                        });
                    }
                });
        });
    }
}

impl eframe::App for WaDesk {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.palette = crate::theme::palette(&self.settings.theme, self.system_dark);
        crate::theme::apply(ctx, &self.palette);
        self.model.now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        // A preview-only self-view image (screenshots without a camera).
        if let Some(path) = self.call_self_source.clone()
            && self.call_self_frame.is_none()
            && let Ok(bytes) = std::fs::read(&path)
            && let Ok(image) = image::load_from_memory(&bytes)
        {
            let image = image.to_rgba8();
            let size = [image.width() as usize, image.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
            self.call_self_frame =
                Some(ctx.load_texture("call-self-preview", color, egui::TextureOptions::LINEAR));
        }
        self.drain_events(ctx);
        self.poll_tray(ctx);
        // A new remote call frame becomes a texture once per generation.
        if let Some(decoder) = &self.call_video {
            let generation = decoder.latest().generation();
            if generation != self.call_frame_generation
                && let Some(frame) = decoder.latest().frame()
            {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [crate::callvideo::WIDTH, crate::callvideo::HEIGHT],
                    &frame,
                );
                self.call_frame =
                    Some(ctx.load_texture("call-frame", image, egui::TextureOptions::LINEAR));
                self.call_frame_generation = generation;
                ctx.request_repaint();
            }
        }
        self.model.now = now_ms();
        // Screenshot testing: open a seeded chat so history can be exercised
        // without clicking.
        if !self.auto_opened
            && self.model.active.is_none()
            && std::env::var("WA_DESK_OPEN_FIRST").is_ok()
            && ctx.input(|input| input.time) > 6.0
        {
            self.auto_opened = true;
            if let Some(jid) = self
                .model
                .visible_chats()
                .first()
                .map(|row| row.jid.clone())
            {
                self.model.open_chat(&jid);
                self.send(Command::ChatHistory { chat: jid });
            }
        }
        if !self.model.login.is_linked() {
            self.gate(ctx);
        } else {
            // Check for a newer release once, quietly, after the gate.
            if !self.update_checked {
                self.update_checked = true;
                self.check_updates();
            }
            if let Some((text, started)) = &self.notice
                && started.elapsed() < std::time::Duration::from_secs(4)
            {
                let text = text.clone();
                let palette = self.palette.clone();
                egui::Area::new(egui::Id::new("notice"))
                    .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 72.0))
                    .interactable(false)
                    .show(ctx, |ui| {
                        egui::Frame::new()
                            .fill(palette.elevated)
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(egui::Margin::symmetric(14, 7))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(text).size(12.5).color(palette.text));
                            });
                    });
            }

            if let Some(call) = self.model.call.clone() {
                self.call_overlay(ctx, &call);
            }

            self.sidebar(ctx);
            if self.show_group {
                self.group_panel(ctx);
            } else if self.show_settings {
                self.settings_panel(ctx);
            } else if self.show_channels {
                self.channels_panel(ctx);
            } else if self.show_status {
                self.status_panel(ctx);
            } else {
                self.conversation(ctx);
                self.emoji_window(ctx);
                self.stickers_window(ctx);
                self.delete_window(ctx);
                self.forward_window(ctx);
            }
            if self.model.media_viewer.is_some() {
                self.media_overlay(ctx);
            }
        }
        let _ = &self.runtime;
        self.chat_menu(ctx);
        self.chat_actions_menu(ctx);
        self.new_chat_window(ctx);
        self.starred_window(ctx);
        self.new_group_window(ctx);
        self.new_contact_window(ctx);
        self.group_add_window(ctx);
        self.screenshot_tick(ctx);
        // A restart only finds the chat list because it is mirrored here: the
        // phone does not replay history to an already-linked device.
        if !self.preview
            && self.cache_dirty
            && self.cache_saved.elapsed() >= std::time::Duration::from_secs(2)
        {
            self.cache_dirty = false;
            self.cache_saved = std::time::Instant::now();
            self.model.save_cache_file(&self.cache_path);
        }
        // Nothing cached and freshly connected: seed from the address book and
        // ask the phone for a small batch of history.
        if self.model.needs_history && self.model.login.is_linked() {
            self.model.needs_history = false;
            self.send(Command::InitialSync);
            self.send(Command::SyncChatState);
            self.send(Command::SyncGroups);
            // Keep pulling batches so the recent chats surface, like the phone.
            self.sync_rounds = 12;
            self.last_sync = std::time::Instant::now();
        }
        // A fresh connection (not just the first) resumes the backfill: rows
        // without history have no timestamp and would otherwise sit at the
        // bottom of the list forever.
        if self.model.connection_epoch != self.seen_epoch {
            self.seen_epoch = self.model.connection_epoch;
            // Every connection refreshes everything the phone can give us:
            // address book + history batches, groups, statuses and chat state.
            self.send(Command::InitialSync);
            self.send(Command::SyncChatState);
            self.send(Command::SyncGroups);
            self.send(Command::StatusHistory);
            self.sync_rounds = 6;
            self.last_sync = std::time::Instant::now();
            self.last_state_sync = std::time::Instant::now();
            self.last_full_sync = std::time::Instant::now();
        }
        // Read marks, mutes and pins live in the chat app-state collection. A
        // slow poll keeps the badges in step with the phone (the official client
        // receives the same patches live, but a missed one would stick forever).
        if self.model.login.is_linked()
            && self.last_state_sync.elapsed() >= std::time::Duration::from_secs(45)
        {
            self.last_state_sync = std::time::Instant::now();
            self.send(Command::SyncChatState);
        }
        // Groups and statuses change slowly; refresh them every ten minutes so
        // the list and the rings keep up without a restart.
        if self.model.login.is_linked()
            && self.last_full_sync.elapsed() >= std::time::Duration::from_secs(600)
        {
            self.last_full_sync = std::time::Instant::now();
            self.send(Command::SyncGroups);
            self.send(Command::StatusHistory);
        }
        if self.model.login.is_linked()
            && self.sync_rounds > 0
            && self.last_sync.elapsed() >= std::time::Duration::from_secs(8)
        {
            self.sync_rounds -= 1;
            self.last_sync = std::time::Instant::now();
            self.send(Command::SyncMore);
        }
        // The session runs on other threads and pushes into a channel; without a
        // timer egui would only repaint on mouse/keyboard input, so the chat list
        // (and new messages) could sit unseen until the window was touched.
        if self.audio.state().playing || self.video_playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}

impl Drop for WaDesk {
    fn drop(&mut self) {
        if !self.preview {
            self.model.save_cache_file(&self.cache_path);
        }
    }
}

impl WaDesk {
    /// The emoji picker: search, recents, and a wrapped grid that inserts into
    /// the composer.
    /// The emoji picker: the official panel - category tabs, a rounded search
    /// field, colour emoji in a nine-column grid and the Emoji / GIF / Stickers
    /// footer. Glyphs come from the system colour font (`emoji_glyph`), so they
    /// look like the phone's rather than egui's monochrome outlines.
    fn emoji_window(&mut self, ctx: &egui::Context) {
        if !self.model.emoji_open {
            return;
        }
        let palette = self.palette.clone();
        let groups = wa_ui::emoji::groups();
        let mut clicked: Option<String> = None;
        let mut close = false;
        let mut stickers = false;

        egui::Area::new(egui::Id::new("emoji-panel"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-96.0, -84.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(364.0);
                        // Category tabs: recents first, then the catalogue.
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            let recent = self.emoji_texture(ui.ctx(), '\u{1F553}', 20);
                            if emoji_category(ui, &palette, "🕘", recent, self.emoji_group == 0)
                                .clicked()
                            {
                                self.emoji_group = 0;
                                self.emoji_gif = false;
                            }
                            for (index, group) in groups.iter().enumerate() {
                                let glyph = group.icon.chars().next().unwrap_or('?');
                                let texture = self.emoji_texture(ui.ctx(), glyph, 20);
                                if emoji_category(
                                    ui,
                                    &palette,
                                    &group.icon,
                                    texture,
                                    self.emoji_group == index + 1 && !self.emoji_gif,
                                )
                                .clicked()
                                {
                                    self.emoji_group = index + 1;
                                    self.emoji_gif = false;
                                }
                            }
                        });
                        ui.add_space(8.0);
                        // Rounded search field.
                        egui::Frame::new()
                            .fill(palette.rail)
                            .corner_radius(egui::CornerRadius::same(9))
                            .inner_margin(egui::Margin::symmetric(10, 5))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(16.0, 16.0),
                                        egui::Sense::hover(),
                                    );
                                    draw_icon(ui, rect, Icon::Search, palette.secondary);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.model.emoji_query)
                                            .frame(false)
                                            .hint_text("Search emoji")
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                            });
                        ui.add_space(6.0);

                        let query = self.model.emoji_query.trim().to_string();
                        let height = (ui.ctx().screen_rect().height() * 0.42).clamp(180.0, 320.0);
                        egui::ScrollArea::vertical()
                            .max_height(height)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                if self.emoji_gif {
                                    ui.add_space(10.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "GIF search is not part of this build yet - the \
                                             sticker tab has the account's stickers.",
                                        )
                                        .size(12.5)
                                        .color(palette.secondary),
                                    );
                                } else if !query.is_empty() {
                                    let items = wa_ui::emoji::search(&query, 160);
                                    let chars: Vec<String> =
                                        items.iter().map(|item| item.char.clone()).collect();
                                    clicked = self.emoji_grid(ui, &palette, &chars);
                                } else if self.emoji_group == 0
                                    && !self.model.emoji_recents.is_empty()
                                {
                                    let recents = self.model.emoji_recents.clone();
                                    clicked = self.emoji_grid(ui, &palette, &recents);
                                } else {
                                    // Recents with nothing remembered yet show the
                                    // first catalogue group, like the phone does.
                                    let index = self.emoji_group.saturating_sub(1);
                                    if let Some(group) = groups.get(index) {
                                        ui.label(
                                            egui::RichText::new(&group.label)
                                                .size(11.5)
                                                .strong()
                                                .color(palette.secondary),
                                        );
                                        ui.add_space(4.0);
                                        let chars: Vec<String> = group
                                            .items
                                            .iter()
                                            .map(|item| item.char.clone())
                                            .collect();
                                        clicked = self.emoji_grid(ui, &palette, &chars);
                                    }
                                }
                            });

                        ui.add_space(6.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 18.0;
                            ui.label(
                                egui::RichText::new("Emoji")
                                    .size(12.5)
                                    .strong()
                                    .color(palette.accent),
                            );
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("GIF")
                                            .size(12.5)
                                            .color(palette.secondary),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                self.emoji_gif = true;
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Stickers")
                                            .size(12.5)
                                            .color(palette.secondary),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                stickers = true;
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new("Close")
                                                    .size(12.0)
                                                    .color(palette.secondary),
                                            )
                                            .frame(false),
                                        )
                                        .clicked()
                                    {
                                        close = true;
                                    }
                                },
                            );
                        });
                    });
            });

        if let Some(emoji) = clicked {
            self.model.insert_emoji(&emoji);
        }
        if stickers {
            self.model.emoji_open = false;
            self.model.stickers_open = true;
            self.send(Command::FavoriteStickers);
        }
        if close {
            self.model.emoji_open = false;
        }
    }

    /// A colour texture for one emoji glyph.
    fn emoji_texture(
        &mut self,
        ctx: &egui::Context,
        ch: char,
        px: u32,
    ) -> Option<egui::TextureHandle> {
        crate::emoji_glyph::texture(ctx, &mut self.emoji_textures, ch, px)
    }

    /// One emoji cell: the colour glyph, a hover pill, click to insert.
    fn emoji_cell(
        &mut self,
        ui: &mut egui::Ui,
        palette: &crate::theme::Palette,
        emoji: &str,
    ) -> bool {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::click());
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(8), palette.hover);
        }
        let glyph = emoji.chars().next().unwrap_or('?');
        if let Some(texture) = self.emoji_texture(ui.ctx(), glyph, 30) {
            let size = texture.size_vec2();
            let scale = (28.0 / size.x.max(1.0)).min(28.0 / size.y.max(1.0));
            ui.painter().image(
                texture.id(),
                egui::Rect::from_center_size(rect.center(), size * scale),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                emoji,
                egui::FontId::proportional(19.0),
                palette.text,
            );
        }
        response.clicked()
    }

    /// The nine-column grid of emoji.
    fn emoji_grid(
        &mut self,
        ui: &mut egui::Ui,
        palette: &crate::theme::Palette,
        chars: &[String],
    ) -> Option<String> {
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(1.0, 1.0);
            for emoji in chars {
                if self.emoji_cell(ui, palette, emoji) {
                    clicked = Some(emoji.clone());
                }
            }
        });
        clicked
    }

    /// The sticker picker: the account's favourites, then the stickers the
    /// session already downloaded from the chats. Clicking sends one.
    /// The sticker picker: the official panel - Recents / Favourites / Packs
    /// tabs, a store search field, a grid of stickers, the Create tile and the
    /// shared Emoji / GIF / Stickers footer.
    fn stickers_window(&mut self, ctx: &egui::Context) {
        if !self.model.stickers_open {
            return;
        }
        let palette = self.palette.clone();
        let chat = self.model.active.clone().unwrap_or_default();
        let favorites = self.model.favorites.clone();
        let error = self.model.favorites_error.clone();
        let recent = self.model.sticker_collection();
        let mut send: Option<Command> = None;
        let mut close = false;
        let mut emoji_tab = false;
        let mut gif_tab = false;

        egui::Area::new(egui::Id::new("sticker-panel"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-96.0, -84.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(364.0);
                        // Tabs: recents, favourites, packs.
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            if sticker_tab(ui, &palette, "Recents", self.sticker_tab == 0).clicked()
                            {
                                self.sticker_tab = 0;
                            }
                            if sticker_tab(ui, &palette, "Favourites", self.sticker_tab == 1)
                                .clicked()
                            {
                                self.sticker_tab = 1;
                            }
                            if sticker_tab(ui, &palette, "Packs", self.sticker_tab == 2).clicked() {
                                self.sticker_tab = 2;
                            }
                        });
                        ui.add_space(8.0);
                        egui::Frame::new()
                            .fill(palette.rail)
                            .corner_radius(egui::CornerRadius::same(9))
                            .inner_margin(egui::Margin::symmetric(10, 5))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(16.0, 16.0),
                                        egui::Sense::hover(),
                                    );
                                    draw_icon(ui, rect, Icon::Search, palette.secondary);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.sticker_query)
                                            .frame(false)
                                            .hint_text("Search stickers")
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                            });
                        ui.add_space(6.0);

                        if chat.is_empty() {
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("Open a chat to send a sticker")
                                    .size(12.5)
                                    .color(palette.secondary),
                            );
                        }
                        if let Some(err) = error {
                            ui.label(
                                egui::RichText::new(format!("Favourites: {err}"))
                                    .size(11.5)
                                    .color(palette.danger),
                            );
                        }

                        let query = self.sticker_query.trim().to_lowercase();
                        let mut rows: Vec<(String, String, bool)> = Vec::new();
                        match self.sticker_tab {
                            1 => {
                                for favorite in &favorites {
                                    rows.push((favorite.id.clone(), favorite.path.clone(), true));
                                }
                            }
                            2 => {}
                            _ => {
                                for (id, path) in &recent {
                                    rows.push((id.clone(), path.clone(), false));
                                }
                            }
                        }
                        if !query.is_empty() {
                            rows.retain(|(_, path, _)| path.to_lowercase().contains(&query));
                        }

                        egui::ScrollArea::vertical()
                            .max_height(300.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                if self.sticker_tab == 2 {
                                    ui.add_space(8.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "Sticker packs are not part of this build yet - \
                                             favourites and recent stickers work.",
                                        )
                                        .size(12.5)
                                        .color(palette.secondary),
                                    );
                                }
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    if self.sticker_tab == 0 {
                                        // The Create tile leads the recents grid.
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(74.0, 74.0),
                                            egui::Sense::click(),
                                        );
                                        if response.hovered() {
                                            ui.painter().rect_filled(
                                                rect,
                                                egui::CornerRadius::same(10),
                                                palette.hover,
                                            );
                                        }
                                        draw_icon(
                                            ui,
                                            egui::Rect::from_center_size(
                                                rect.center(),
                                                egui::vec2(26.0, 26.0),
                                            ),
                                            Icon::Add,
                                            palette.secondary,
                                        );
                                        if response.clicked() {
                                            self.notice = Some((
                                                "Creating stickers is not supported yet."
                                                    .to_string(),
                                                std::time::Instant::now(),
                                            ));
                                        }
                                    }
                                    for (id, path, favorite) in &rows {
                                        let key = format!("sticker:{id}");
                                        let Some(texture) = texture_at(
                                            ui.ctx(),
                                            &mut self.sticker_textures,
                                            &key,
                                            Some(path.clone()),
                                        ) else {
                                            continue;
                                        };
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(74.0, 74.0),
                                            egui::Sense::click(),
                                        );
                                        if response.hovered() {
                                            ui.painter().rect_filled(
                                                rect,
                                                egui::CornerRadius::same(10),
                                                palette.hover,
                                            );
                                        }
                                        let size = texture.size_vec2();
                                        let scale =
                                            (66.0 / size.x.max(1.0)).min(66.0 / size.y.max(1.0));
                                        ui.painter().image(
                                            texture.id(),
                                            egui::Rect::from_center_size(
                                                rect.center(),
                                                size * scale,
                                            ),
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            egui::Color32::WHITE,
                                        );
                                        if response.clicked() && !chat.is_empty() {
                                            send = Some(if *favorite {
                                                Command::SendFavoriteSticker {
                                                    chat: chat.clone(),
                                                    id: id.clone(),
                                                }
                                            } else {
                                                Command::ForwardSticker {
                                                    chat: chat.clone(),
                                                    message_id: id.clone(),
                                                }
                                            });
                                        }
                                    }
                                });
                                if rows.is_empty() && self.sticker_tab != 2 {
                                    ui.add_space(10.0);
                                    ui.label(
                                        egui::RichText::new(if self.sticker_tab == 1 {
                                            "No favourite stickers yet."
                                        } else {
                                            "Send or receive a sticker and it shows up here."
                                        })
                                        .size(12.5)
                                        .color(palette.secondary),
                                    );
                                }
                            });

                        ui.add_space(6.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 18.0;
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Emoji")
                                            .size(12.5)
                                            .color(palette.secondary),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                emoji_tab = true;
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("GIF")
                                            .size(12.5)
                                            .color(palette.secondary),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                gif_tab = true;
                            }
                            ui.label(
                                egui::RichText::new("Stickers")
                                    .size(12.5)
                                    .strong()
                                    .color(palette.accent),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new("Close")
                                                    .size(12.0)
                                                    .color(palette.secondary),
                                            )
                                            .frame(false),
                                        )
                                        .clicked()
                                    {
                                        close = true;
                                    }
                                },
                            );
                        });
                    });
            });

        if let Some(command) = send {
            self.send(command);
            // One tap sends one sticker; the panel closes immediately like the
            // official picker instead of letting a slow upload double-send.
            self.model.stickers_open = false;
        }
        if emoji_tab {
            self.model.stickers_open = false;
            self.model.emoji_open = true;
            self.emoji_gif = false;
        }
        if gif_tab {
            self.model.stickers_open = false;
            self.model.emoji_open = true;
            self.emoji_gif = true;
        }
        if close {
            self.model.stickers_open = false;
        }
    }

    /// The delete dialog: for me, and for everyone when `wa-ui` allows it.
    fn delete_window(&mut self, ctx: &egui::Context) {
        if self.model.delete_target.is_none() {
            return;
        }
        let (title, can_everyone) = self.model.delete_question();
        let target = self.model.delete_target.clone();
        let mut open = true;
        egui::Window::new("Delete message")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.label(title);
                let Some(target) = target.clone() else { return };
                if ui.button("Delete for me").clicked() {
                    self.send(Command::DeleteForMe {
                        chat: target.chat.clone(),
                        id: target.id.clone(),
                        from_me: target.from_me,
                        participant: if target.from_me {
                            None
                        } else {
                            target.sender.clone()
                        },
                    });
                    self.model.remove_message(&target.chat, &target.id);
                    self.model.delete_target = None;
                }
                if can_everyone && ui.button("Delete for everyone").clicked() {
                    self.send(Command::DeleteForEveryone {
                        chat: target.chat.clone(),
                        id: target.id.clone(),
                    });
                    self.model.delete_target = None;
                }
                if ui.button("Cancel").clicked() {
                    self.model.delete_target = None;
                }
            });
        if !open {
            self.model.delete_target = None;
        }
    }

    /// The forward picker: search, multi-select, then one send for all chats.
    fn forward_window(&mut self, ctx: &egui::Context) {
        let Some(target) = self.model.forward_target.clone() else {
            return;
        };
        let mut open = true;
        egui::Window::new("Forward to")
            .open(&mut open)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.model.forward_search)
                        .hint_text("Search chats")
                        .desired_width(f32::INFINITY),
                );
                let rows: Vec<(String, String)> = self
                    .model
                    .forward_rows()
                    .iter()
                    .map(|row| (row.jid.clone(), self.model.title(row)))
                    .collect();
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        for (jid, name) in rows {
                            let mut chosen = self.model.forward_chosen.contains(&jid);
                            if ui.checkbox(&mut chosen, name).changed() {
                                self.model.toggle_forward(&jid);
                            }
                        }
                    });
                ui.horizontal(|ui| {
                    let count = self.model.forward_chosen.len();
                    if ui
                        .add_enabled(count > 0, egui::Button::new(format!("Send to {count}")))
                        .clicked()
                    {
                        self.send(Command::Forward {
                            chats: self.model.forward_chosen.clone(),
                            id: target.id.clone(),
                        });
                        self.model.forward_target = None;
                    }
                    if ui.button("Cancel").clicked() {
                        self.model.forward_target = None;
                    }
                });
            });
        if !open {
            self.model.forward_target = None;
        }
    }

    /// The media viewer: a full-window stage, the filmstrip, prev/next, Esc to
    /// close, arrow keys to step, and Save into `Downloads\WA Desk`.
    fn media_overlay(&mut self, ctx: &egui::Context) {
        let Some(item) = self.model.viewer_item() else {
            self.model.close_viewer();
            return;
        };
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.stop_video();
            self.model.close_viewer();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            self.stop_video();
            self.model.step_viewer(true);
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            self.stop_video();
            self.model.step_viewer(false);
            return;
        }
        if item.kind == "video" && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.toggle_video(&item.id);
        }
        if item.kind == "video"
            && !self.videos.contains_key(&item.id)
            && let Some(path) = self.model.media_path(&item.id)
        {
            self.videos.insert(item.id.clone(), VideoState::Preparing);
            diag(&format!("video prepare {} <- {path}", item.id));
            let tx = self.video_tx.clone();
            let id = item.id.clone();
            std::thread::spawn(move || {
                let reply = crate::video::prepare(&path, &id);
                let _ = tx.send((id, reply));
            });
        }
        // Reached the end: park on the last frame.
        if item.kind == "video"
            && self.preview_autoplay
            && !self.video_playing
            && self.video_elapsed() < 0.5
            && matches!(self.videos.get(&item.id), Some(VideoState::Ready(_)))
        {
            self.toggle_video(&item.id);
        }
        if item.kind == "video"
            && self.video_playing
            && let Some(VideoState::Ready(video)) = self.videos.get(&item.id)
            && self.video_elapsed() >= video.duration
        {
            self.video_offset = video.duration;
            self.video_playing = false;
            self.video_started = None;
            self.audio.stop();
        }
        if self.model.request_media(&item.id) {
            self.send(Command::FetchMedia {
                id: item.id.clone(),
            });
        }

        let items = self.model.media_items();
        let index = self.model.media_viewer.unwrap_or(0);
        let counter = format!("{} / {}", index + 1, items.len());
        let chat_name = self
            .model
            .chats
            .get(&item.chat)
            .map(|row| self.model.title(row))
            .unwrap_or_default();
        let stamp = self.model.bubble_stamp(&item);
        let texture = texture_at(
            ctx,
            &mut self.textures,
            &item.id,
            self.model.media_path(&item.id),
        );
        let thumbs: Vec<(String, Option<egui::TextureHandle>)> = items
            .iter()
            .map(|entry| {
                let texture = texture_at(
                    ctx,
                    &mut self.textures,
                    &entry.id,
                    self.model.media_path(&entry.id),
                );
                (entry.id.clone(), texture)
            })
            .collect();

        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("media-viewer"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(11, 20, 26))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_min_size(screen.size() - egui::vec2(24.0, 24.0));
                        ui.horizontal(|ui| {
                            let white = egui::Color32::from_gray(240);
                            ui.label(egui::RichText::new(&chat_name).strong().color(white));
                            ui.label(egui::RichText::new(&stamp).small().weak());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if viewer_icon(ui, Icon::Close).clicked() {
                                        self.model.close_viewer();
                                    }
                                    if viewer_icon(ui, Icon::Download).clicked() {
                                        self.note = match self.model.save_media(&item.id) {
                                            Ok(path) => format!("Saved to {path}"),
                                            Err(err) => err,
                                        };
                                    }
                                    ui.label(egui::RichText::new(&counter).small().weak());
                                },
                            );
                        });

                        // An `Area`'s `available_height` is effectively unbounded
                        // (83k px here), which pushed the stage off screen, so the
                        // stage is measured from the window instead: margins,
                        // header, transport bar and the caption/filmstrip row.
                        let bar_height = if item.kind == "video" { 158.0 } else { 124.0 };
                        let stage_height = (screen.height() - 24.0 - 40.0 - bar_height).max(160.0);
                        let mut stage_clicked = false;
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), stage_height),
                            egui::Layout::centered_and_justified(egui::Direction::TopDown),
                            |ui| {
                                if item.kind == "video" {
                                    match self.video_frame_handle(ctx, &item.id) {
                                        Some(frame) => {
                                            let size = frame.size_vec2();
                                            // Fill the stage like the official
                                            // player (frames are decoded 640 wide).
                                            let scale = (ui.available_width() / size.x)
                                                .min(ui.available_height() / size.y);
                                            let response = ui.add(
                                                egui::Image::new((frame.id(), size * scale))
                                                    .sense(egui::Sense::click()),
                                            );
                                            stage_clicked = response.clicked();
                                            if !self.video_playing {
                                                // Paused, like the official viewer: a dim
                                                // scrim with a big translucent play disc.
                                                let rect = response.rect;
                                                let painter = ui.painter();
                                                painter.rect_filled(
                                                    rect,
                                                    egui::CornerRadius::same(2),
                                                    egui::Color32::from_black_alpha(70),
                                                );
                                                painter.circle_filled(
                                                    rect.center(),
                                                    32.0,
                                                    egui::Color32::from_black_alpha(130),
                                                );
                                                draw_icon(
                                                    ui,
                                                    egui::Rect::from_center_size(
                                                        rect.center(),
                                                        egui::vec2(30.0, 30.0),
                                                    ),
                                                    Icon::Play,
                                                    egui::Color32::from_gray(240),
                                                );
                                            }
                                        }
                                        None => {
                                            let text = match self.videos.get(&item.id) {
                                                Some(VideoState::Failed(err)) => {
                                                    format!("Cannot play: {err}")
                                                }
                                                Some(VideoState::Ready(_)) => {
                                                    "Loading frame.".to_string()
                                                }
                                                _ => "Preparing video.".to_string(),
                                            };
                                            ui.label(
                                                egui::RichText::new(text)
                                                    .color(egui::Color32::from_gray(200)),
                                            );
                                        }
                                    }
                                    return;
                                }
                                match &texture {
                                    Some(texture) => {
                                        let size = texture.size_vec2();
                                        let scale = (ui.available_width() / size.x)
                                            .min(ui.available_height() / size.y)
                                            .min(1.0);
                                        ui.image((texture.id(), size * scale));
                                    }
                                    None => {
                                        ui.label(
                                            egui::RichText::new("Loading.")
                                                .color(egui::Color32::from_gray(200)),
                                        );
                                    }
                                }
                            },
                        );
                        if stage_clicked {
                            self.toggle_video(&item.id);
                        }

                        if item.kind == "video" {
                            self.video_transport(ui, ctx, &item.id);
                        }

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                if !item.text.trim().is_empty() {
                                    ui.label(
                                        egui::RichText::new(&item.text)
                                            .color(egui::Color32::from_gray(220)),
                                    );
                                }
                                ui.horizontal(|ui| {
                                    if viewer_icon(ui, Icon::ChevronLeft).clicked() {
                                        self.stop_video();
                                        self.model.step_viewer(false);
                                    }
                                    if viewer_icon(ui, Icon::ChevronRight).clicked() {
                                        self.stop_video();
                                        self.model.step_viewer(true);
                                    }
                                    if !self.note.is_empty() {
                                        ui.label(egui::RichText::new(&self.note).small().weak());
                                    }
                                });
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    for (id, thumb) in thumbs.iter().rev() {
                                        let Some(thumb) = thumb else { continue };
                                        let size = thumb.size_vec2();
                                        let scale = (44.0 / size.x).min(1.0);
                                        let response = ui.add(
                                            egui::Image::new((thumb.id(), size * scale))
                                                .sense(egui::Sense::click()),
                                        );
                                        if response.clicked() {
                                            self.model.open_viewer(id);
                                        }
                                    }
                                },
                            );
                        });
                    });
            });
    }
}

/// Trim `text` until it measures inside `max_width`, adding an ellipsis.
fn fit_text(
    painter: &egui::Painter,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
    max_width: f32,
) -> String {
    if max_width <= 0.0 {
        return String::new();
    }
    if painter
        .layout_no_wrap(text.to_string(), font.clone(), color)
        .size()
        .x
        <= max_width
    {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut len = chars.len();
    while len > 1 {
        len -= 1;
        let mut candidate: String = chars[..len].iter().collect();
        candidate.push('…');
        if painter
            .layout_no_wrap(candidate.clone(), font.clone(), color)
            .size()
            .x
            <= max_width
        {
            return candidate;
        }
    }
    "…".to_string()
}

/// The icons the window draws itself. Emoji glyphs are not in egui's bundled
/// fonts (they render as boxes), so every icon is a few strokes instead.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Icon {
    Search,
    Menu,
    Status,
    Channels,
    Settings,
    Emoji,
    Sticker,
    Send,
    Reply,
    Forward,
    Copy,
    Edit,
    Delete,
    Star,
    GroupAdd,
    PersonAdd,
    Document,
    Attach,
    Chats,
    Add,
    Groups,
    Logout,
    Play,
    Pause,
    Close,
    Download,
    Volume,
    Mute,
    Fullscreen,
    FullscreenExit,
    ChevronLeft,
    ChevronRight,
    Phone,
    CallEnd,
    VideoCall,
    Mic,
    SelectChats,
    MarkRead,
    Lock,
}

fn icon_button(ui: &mut egui::Ui, icon: Icon, palette: &crate::theme::Palette) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(16), palette.hover);
    }
    let color = if response.hovered() {
        palette.text
    } else {
        palette.secondary
    };
    draw_icon(ui, rect.shrink(6.0), icon, color);
    response
}

/// A white icon button for the dark media-viewer chrome.
fn viewer_icon(ui: &mut egui::Ui, icon: Icon) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .circle_filled(rect.center(), 17.0, egui::Color32::from_white_alpha(34));
    }
    let color = if response.hovered() {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_gray(225)
    };
    draw_icon(ui, rect.shrink(8.0), icon, color);
    response
}

/// Paint one Material Symbol inside `rect`; the texture is cached per colour.
fn draw_icon(ui: &egui::Ui, rect: egui::Rect, icon: Icon, color: egui::Color32) {
    let size = rect.width().round().max(8.0) as u32;
    if let Some(texture) = crate::icons::texture(ui.ctx(), icon.name(), size, color) {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
}

impl Icon {
    /// The Material Symbols name for this icon.
    fn name(self) -> &'static str {
        match self {
            Icon::Search => "search",
            Icon::Menu => "more_vert",
            Icon::Status => "donut_large",
            Icon::Channels => "campaign",
            Icon::Settings => "settings",
            Icon::Emoji => "mood",
            Icon::Sticker => "sticker",
            Icon::Send => "send",
            Icon::Reply => "reply",
            Icon::Forward => "shortcut",
            Icon::Copy => "content_copy",
            Icon::Edit => "edit",
            Icon::Delete => "delete",
            Icon::Star => "star",
            Icon::GroupAdd => "group_add",
            Icon::PersonAdd => "person_add",
            Icon::Document => "description",
            Icon::Attach => "attach_file",
            Icon::Chats => "chat_bubble",
            Icon::Add => "add",
            Icon::Groups => "groups",
            Icon::Logout => "logout",
            Icon::Play => "play_arrow",
            Icon::Pause => "pause",
            Icon::Close => "close",
            Icon::Download => "download",
            Icon::Volume => "volume_up",
            Icon::Mute => "volume_off",
            Icon::Fullscreen => "fullscreen",
            Icon::FullscreenExit => "fullscreen_exit",
            Icon::ChevronLeft => "chevron_left",
            Icon::ChevronRight => "chevron_right",
            Icon::Phone => "call",
            Icon::CallEnd => "call_end",
            Icon::VideoCall => "videocam",
            Icon::Mic => "mic",
            Icon::SelectChats => "done_all",
            Icon::MarkRead => "mark_chat_read",
            Icon::Lock => "lock",
        }
    }
}

/// An outlined filter pill (`All`, `Groups 3`, …), filled when selected.
fn pill_button(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    label: &str,
    selected: bool,
) -> bool {
    let font = egui::FontId::proportional(11.5);
    let text = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), palette.text);
    let width = text.size().x + 20.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 27.0), egui::Sense::click());
    let fill = if selected {
        palette.selected
    } else if response.hovered() {
        palette.hover
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(14),
        fill,
        egui::Stroke::new(1.0_f32, palette.border),
        egui::StrokeKind::Inside,
    );
    let color = if selected {
        palette.text
    } else {
        palette.secondary
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        color,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// One rail button, with an optional badge for unread counts.
fn rail_button(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    icon: Icon,
    selected: bool,
    badge: i64,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(46.0, 46.0), egui::Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(12),
            if selected {
                palette.selected
            } else {
                palette.hover
            },
        );
    }
    let color = if selected {
        palette.accent
    } else {
        palette.secondary
    };
    draw_icon(
        ui,
        egui::Rect::from_center_size(rect.center(), egui::vec2(24.0, 24.0)),
        icon,
        color,
    );
    if badge > 0 {
        let label = if badge > 99 {
            "99+".to_string()
        } else {
            badge.to_string()
        };
        let width = 14.0 + label.len() as f32 * 3.5;
        let pill = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 4.0, rect.top() + 10.0),
            egui::vec2(width, 16.0),
        );
        ui.painter().rect_filled(pill, 8.0, palette.unread);
        ui.painter().text(
            pill.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(10.0),
            egui::Color32::WHITE,
        );
    }
    response
}

/// The green "new chat" floating button in the Chats header.
fn fab_button(ui: &mut egui::Ui, palette: &crate::theme::Palette) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
    let fill = if response.hovered() {
        palette.accent.linear_multiply(1.15)
    } else {
        palette.accent
    };
    ui.painter().circle_filled(rect.center(), 20.0, fill);
    let ink = if palette.dark {
        egui::Color32::from_rgb(11, 20, 26)
    } else {
        egui::Color32::WHITE
    };
    draw_icon(
        ui,
        egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 20.0)),
        Icon::Add,
        ink,
    );
    response
}

/// One of the round empty-state actions (circle + caption).
fn action_button(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    icon: Icon,
    label: &str,
) -> bool {
    ui.vertical(|ui| {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(72.0, 64.0), egui::Sense::click());
        let fill = if response.hovered() {
            palette.hover
        } else {
            palette.elevated
        };
        ui.painter().circle_filled(rect.center(), 28.0, fill);
        draw_icon(
            ui,
            egui::Rect::from_center_size(
                egui::pos2(rect.center().x, rect.center().y - 2.0),
                egui::vec2(22.0, 22.0),
            ),
            icon,
            palette.secondary,
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(label)
                .size(11.5)
                .color(palette.secondary),
        );
        response.clicked()
    })
    .inner
}

/// A round initials avatar, like WhatsApp's default profile photo.
fn paint_avatar(ui: &mut egui::Ui, name: &str, size: f32, texture: Option<&egui::TextureHandle>) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter();
    if let Some(texture) = texture {
        painter.image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        return;
    }
    painter.circle_filled(rect.center(), size * 0.5, avatar_color(name));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials(name),
        egui::FontId::proportional(size * 0.38),
        egui::Color32::WHITE,
    );
}

/// Decode a profile photo into a circular avatar texture.
fn circular_avatar(ctx: &egui::Context, key: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    let side = width.min(height).max(1);
    let x0 = (width - side) / 2;
    let y0 = (height - side) / 2;
    let radius = side as f32 / 2.0;
    let mut pixels = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let pixel = image.get_pixel(x0 + x, y0 + y);
            let dx = x as f32 + 0.5 - radius;
            let dy = y as f32 + 0.5 - radius;
            let alpha = if dx * dx + dy * dy <= radius * radius {
                pixel[3]
            } else {
                0
            };
            pixels.extend_from_slice(&[pixel[0], pixel[1], pixel[2], alpha]);
        }
    }
    let color = egui::ColorImage::from_rgba_unmultiplied([side as usize, side as usize], &pixels);
    Some(ctx.load_texture(key, color, egui::TextureOptions::LINEAR))
}

fn initials(name: &str) -> String {
    let mut letters = name
        .split_whitespace()
        .filter_map(|word| word.chars().next());
    match (letters.next(), letters.next()) {
        (Some(first), Some(second)) => format!("{first}{second}").to_uppercase(),
        (Some(first), None) => first.to_uppercase().to_string(),
        _ => "?".to_string(),
    }
}

/// The default-avatar palette WhatsApp uses for people without a photo.
fn avatar_color(name: &str) -> egui::Color32 {
    const COLORS: [u32; 8] = [
        0x8E9BA6, 0xB5766B, 0x7C8FA8, 0x9A7EA8, 0xA48A5B, 0x5F8A76, 0xA8708A, 0x5C7E8A,
    ];
    hex_color(COLORS[hash(name) % COLORS.len()])
}

/// Group sender names are coloured per sender, like the official client.
fn sender_color(name: &str, palette: &crate::theme::Palette) -> egui::Color32 {
    const COLORS: [u32; 8] = [
        0x53BDEB, 0x21C063, 0xF5A524, 0xE17E7E, 0xB39DDB, 0x7FD1C1, 0xEBA36A, 0x9FC7E8,
    ];
    let color = hex_color(COLORS[hash(name) % COLORS.len()]);
    if palette.dark {
        color
    } else {
        // Darken for the light theme so the names stay readable on white.
        egui::Color32::from_rgb(color.r() * 3 / 4, color.g() * 3 / 4, color.b() * 4 / 5)
    }
}

fn hex_color(value: u32) -> egui::Color32 {
    egui::Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn hash(text: &str) -> usize {
    text.bytes().fold(0u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(byte as u32)
    }) as usize
}

/// One chat row: avatar, name, preview, clock and the unread pill.
#[allow(clippy::too_many_arguments)]
fn chat_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    row: &ChatRow,
    title: &str,
    preview: &str,
    stamp: &str,
    selected: bool,
    texture: Option<&egui::TextureHandle>,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 72.0), egui::Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 0.0, palette.selected);
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, palette.hover);
    }

    let center = egui::pos2(rect.left() + 40.0, rect.center().y);
    if let Some(texture) = texture {
        painter.image(
            texture.id(),
            egui::Rect::from_center_size(center, egui::vec2(48.0, 48.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        painter.circle_filled(center, 24.0, avatar_color(title));
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            initials(title),
            egui::FontId::proportional(15.0),
            egui::Color32::WHITE,
        );
    }

    let left = rect.left() + 76.0;
    let right = rect.right() - 12.0;
    // Reserve the clock's width so a long name cannot run underneath it.
    let stamp_width = if stamp.is_empty() {
        0.0
    } else {
        painter
            .layout_no_wrap(
                stamp.to_string(),
                egui::FontId::proportional(11.5),
                palette.secondary,
            )
            .size()
            .x
            + 10.0
    };
    painter.text(
        egui::pos2(left, rect.top() + 22.0),
        egui::Align2::LEFT_CENTER,
        fit_text(
            painter,
            title,
            egui::FontId::proportional(15.0),
            palette.text,
            right - left - stamp_width,
        ),
        egui::FontId::proportional(15.0),
        palette.text,
    );
    if !stamp.is_empty() {
        let color = if row.unread > 0 {
            palette.accent
        } else {
            palette.secondary
        };
        painter.text(
            egui::pos2(right, rect.top() + 22.0),
            egui::Align2::RIGHT_CENTER,
            stamp,
            egui::FontId::proportional(11.5),
            color,
        );
    }
    painter.text(
        egui::pos2(left, rect.top() + 47.0),
        egui::Align2::LEFT_CENTER,
        fit_text(
            painter,
            preview,
            egui::FontId::proportional(13.0),
            palette.secondary,
            right - left,
        ),
        egui::FontId::proportional(13.0),
        palette.secondary,
    );
    if row.unread > 0 {
        let pill = egui::Rect::from_center_size(
            egui::pos2(right - 4.0, rect.bottom() - 16.0),
            egui::vec2(20.0, 18.0),
        );
        painter.rect_filled(pill, 9.0, palette.unread);
        painter.text(
            pill.center(),
            egui::Align2::CENTER_CENTER,
            row.unread.to_string(),
            egui::FontId::proportional(11.0),
            egui::Color32::WHITE,
        );
    }
    response.clicked()
}

#[cfg(test)]
mod notification_tests {
    use super::*;

    #[test]
    fn media_notifications_name_the_kind() {
        assert_eq!(notification_body("image", ""), "Photo");
        assert_eq!(notification_body("audio", "   "), "Voice message");
        assert_eq!(notification_body("document", ""), "Document");
        assert_eq!(notification_body("text", "  hi there "), "hi there");
    }
}
