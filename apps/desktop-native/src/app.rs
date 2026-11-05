//! The window: a chat list on the left, the open conversation on the right.
//!
//! Painting is all this module does. Every string it shows comes from
//! `crate::model`, which in turn asks `wa-ui`.

use std::sync::mpsc::Sender;

use tokio::sync::mpsc::UnboundedReceiver;
use wa_types::{MessageView, UiEvent};

use crate::bubbles::{PHOTO_PX, QUOTE_PX, STICKER_PX, bubble, day_chip, menu_row, texture_at};
use crate::login::LoginState;
use crate::login_ui::{
    demo_qr_svg, draw_laptop, draw_whatsapp_mark, gate_card, gate_link, gate_step,
};
use crate::model::{BubbleAction, ChatRow, Command, Model};
use crate::settings::NativeSettings;
use crate::settings_ui::{
    settings_card, settings_field, settings_file, settings_link, settings_list_row,
    settings_radio_row, settings_row, settings_switch,
};
use crate::updates::{self, Release, UpdateMsg, UpdateState};
use crate::widgets::{
    avatar_color, dialog_button, dialog_card, fit_text, hex_colour, initials, paint_avatar,
    single_line, status_send_button, status_share_tile,
};

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
    /// The calls panel is showing instead of a conversation.
    show_calls: bool,
    /// The Communities screen, and the community opened in it.
    show_communities: bool,
    communities_active: Option<String>,
    /// The voice note being recorded from the composer's microphone button.
    voice: Option<crate::voicenote::Recording>,
    /// The attach menu is open.
    show_attach: bool,
    /// Poll and contact-card drafts (the attach menu's two dialogs).
    poll_draft: Option<crate::app::PollDraft>,
    contact_draft: Option<crate::app::ContactDraft>,
    location_draft: Option<crate::app::LocationDraft>,
    /// The name being typed for a new custom list.
    list_draft: Option<String>,
    /// `WA_DESK_SEND_ONCE` has fired (a smoke test of the send path).
    smoke_sent: bool,
    /// `WA_DESK_SCROLL_TOP` parks the conversation at its first message.
    preview_scroll_top: bool,
    /// Text being typed into the share-status box, plus the chosen background
    /// (an index into `wa_ui::composer::BACKGROUNDS`) and font.
    status_compose: Option<String>,
    status_compose_background: usize,
    status_compose_font: usize,
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
    /// The login gate is showing the phone-number form instead of the QR.
    gate_phone: bool,
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
    /// When the last `memory:` line was written (diagnostics only).
    last_memory_log: std::time::Instant,
    /// Paint clock for the `paint:` diagnostic: the previous frame's time and
    /// the interval seen since the last report.
    last_paint: std::time::Instant,
    paint_gap_sum: f32,
    paint_gap_count: u32,
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
    /// The clip whose frames are being decoded ahead, keyed by its directory.
    video_prefetch: Option<(String, crate::frames::Prefetch)>,
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
    /// True once the Packs tab asked the session for its packs.
    packs_requested: bool,
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
        // The media paths from the last run: stickers, photos and posters paint
        // from disk immediately instead of downloading again.
        let media_cache = cache_path.with_file_name("media.json");
        if let Ok(text) = std::fs::read_to_string(&media_cache) {
            model.load_media_cache(&text);
        }
        Self {
            model,
            events: native.events,
            commands: native.commands,
            runtime: native.runtime,
            typing_sent: false,
            show_status: false,
            show_calls: false,
            show_communities: false,
            communities_active: None,
            voice: None,
            show_attach: std::env::var("WA_DESK_ATTACH").is_ok(),
            poll_draft: None,
            contact_draft: None,
            location_draft: None,
            list_draft: None,
            smoke_sent: false,
            preview_scroll_top: std::env::var("WA_DESK_SCROLL_TOP").is_ok(),
            status_compose: None,
            status_compose_background: 0,
            status_compose_font: 0,
            show_channels: false,
            show_settings: false,
            settings,
            settings_path: settings_file(),
            settings_note: String::new(),
            qr: None,
            gate_phone: false,
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
            last_memory_log: std::time::Instant::now(),
            last_paint: std::time::Instant::now(),
            paint_gap_sum: 0.0,
            paint_gap_count: 0,
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
            video_prefetch: None,
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
            packs_requested: false,
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
        // `WA_DESK_LOGIN=qr` (or `pair`) renders the login gate for a
        // screenshot; the QR is generated from a dummy string.
        if std::env::var("WA_DESK_LOGIN").is_ok() {
            self.model.login = match std::env::var("WA_DESK_LOGIN").as_deref() {
                Ok("pair") => crate::login::LoginState::PairCode("ABCD-EFGH".to_string()),
                _ => crate::login::LoginState::Qr(demo_qr_svg()),
            };
        }
        // Opening through the model keeps the unread anchor (and clears the
        // badge), which a screenshot of the divider needs.
        if std::env::var("WA_DESK_OPEN_FIRST").is_ok()
            && self.model.active.is_none()
            && let Some(first) = self
                .model
                .visible_chats()
                .first()
                .map(|row| row.jid.clone())
        {
            self.model.open_chat(&first);
            self.auto_opened = true;
        }
        self.show_group = false;
        self.show_status = false;
        self.show_channels = false;
        // `WA_DESK_SCREEN=status|channels|group` opens one of the secondary
        // screens for a screenshot, so each can be compared with the official
        // client (the settings screen has its own hook below).
        if let Ok(screen) = std::env::var("WA_DESK_SCREEN") {
            match screen.as_str() {
                "calls" => self.show_calls = true,
                "communities" => self.show_communities = true,
                "status" => self.show_status = true,
                "status-compose" => {
                    self.show_status = true;
                    self.status_compose = Some("Good morning!".to_string());
                }
                "channels" => self.show_channels = true,
                "group" => {
                    self.model.active = Some("1203@g.us".to_string());
                    self.show_group = true;
                }
                _ => {}
            }
        }
        // `WA_DESK_SETTINGS=1` renders the Settings screen for a screenshot, and
        // `WA_DESK_SETTINGS_SECTION=n` picks the page shown.
        self.show_settings = std::env::var("WA_DESK_SETTINGS").is_ok();
        if let Ok(section) = std::env::var("WA_DESK_SETTINGS_SECTION")
            && let Ok(index) = section.parse::<usize>()
        {
            self.settings_section = index.min(8);
        }
        // `WA_DESK_POLL=1` / `WA_DESK_CONTACT=1` put the two new message kinds in
        // the demo group so their bubbles can be captured.
        if std::env::var("WA_DESK_STATUS_VIEWER").is_ok() {
            self.show_status = true;
            if let Some(first) = self.model.status_groups().first() {
                let jid = first.jid.clone();
                self.model.open_status(&jid);
            }
        }
        if let Ok(sticker) = std::env::var("WA_DESK_STICKER_SENT") {
            let chat = "1203@g.us".to_string();
            self.model.active = Some(chat.clone());
            self.model.add_own_sticker(&chat, &sticker);
        }
        if std::env::var("WA_DESK_SEND_FAIL").is_ok() {
            let chat = "1203@g.us".to_string();
            self.model.active = Some(chat.clone());
            self.model
                .add_own_text(&chat, "This one never left the socket.", None);
            self.model.apply(&UiEvent::SendFailed {
                chat,
                error: "send failed: not connected".to_string(),
            });
        }
        if std::env::var("WA_DESK_THUMB").is_ok() {
            self.model.active = Some("1203@g.us".to_string());
            self.model.demo_photo_thumb("1203@g.us", "thumb-demo");
        }
        if std::env::var("WA_DESK_MARKUP").is_ok() {
            self.model.active = Some("1203@g.us".to_string());
            self.model.demo_note(
                "1203@g.us",
                "markup-demo",
                "Waduh benar, tadi cuma sampai 7 \u{1F601} Ini nomor 8. **Responsif & konsisten di semua ukuran layar**  Pastikan layout rapi di HP, tablet, dan desktop. Jangan ada teks/tombol kepotong atau jarak berantakan saat layar berubah. Mau aku lanjut kasih checklist UI rapi versi lengkap? \u{1F60A}",
                false,
            );
        }
        if std::env::var("WA_DESK_SENT").is_ok() {
            let chat = "1203@g.us".to_string();
            self.model.active = Some(chat.clone());
            self.model.add_own_text(
                &chat,
                "Sent from this window: the bubble is here before the stanza leaves.",
                None,
            );
        }
        if std::env::var("WA_DESK_FORWARD").is_ok() {
            let chat = "1203@g.us".to_string();
            self.model.active = Some(chat.clone());
            let target = self.model.chats.get(&chat).and_then(|row| {
                row.messages
                    .iter()
                    .find(|message| message.kind == "text")
                    .cloned()
            });
            if let Some(message) = target {
                self.model.start_forward(&message);
            }
            self.model.forward_chosen = ["1@s.whatsapp.net".to_string()].to_vec();
            self.model.forward_search.clear();
        }
        if std::env::var("WA_DESK_LOCATION").is_ok() {
            self.model
                .demo_location("1203@g.us", "location-demo", "Monas", false);
            self.model.active = Some("1203@g.us".to_string());
        }
        if std::env::var("WA_DESK_POLL_DIALOG").is_ok() {
            self.poll_draft = Some(PollDraft {
                question: "Where do we eat on Friday?".to_string(),
                options: [
                    "Warung nasi".to_string(),
                    "Pizza".to_string(),
                    "Sushi".to_string(),
                ],
                multiple: false,
            });
        }
        if std::env::var("WA_DESK_CONTACT_DIALOG").is_ok() {
            self.contact_draft = Some(ContactDraft {
                name: "Ada Lovelace".to_string(),
                phone: "628111000111".to_string(),
            });
        }
        if std::env::var("WA_DESK_POLL").is_ok() {
            self.model.demo_poll(
                "1203@g.us",
                "poll-demo",
                "Where do we eat on Friday?",
                &["Warung nasi", "Pizza", "Sushi"],
            );
            self.model.active = Some("1203@g.us".to_string());
        }
        if std::env::var("WA_DESK_CONTACT").is_ok() {
            self.model
                .demo_contact("1203@g.us", "contact-demo", "Ada Lovelace", "628111000111");
            self.model.active = Some("1203@g.us".to_string());
        }
        // `WA_DESK_STICKER_WEBP=<file.webp>` puts a real (animated) sticker in
        // the demo group, so the animation can be checked frame by frame.
        if let Ok(path) = std::env::var("WA_DESK_STICKER_WEBP") {
            self.model.demo_sticker("1203@g.us", "sticker-webp", &path);
            self.model.active = Some("1203@g.us".to_string());
        }
        // `WA_DESK_GIFS=<dir>` seeds GIF messages and opens the GIF tab.
        if let Ok(dir) = std::env::var("WA_DESK_GIFS") {
            let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|entry| entry.path())
                        .filter(|path| {
                            path.extension()
                                .and_then(|ext| ext.to_str())
                                .is_some_and(|ext| {
                                    // PNG posters or real clips (the bubble animates those).
                                    ["png", "mp4", "webm", "gif"]
                                        .iter()
                                        .any(|kind| ext.eq_ignore_ascii_case(kind))
                                })
                        })
                        .collect()
                })
                .unwrap_or_default();
            paths.sort();
            for (index, path) in paths.iter().enumerate() {
                self.model.demo_gif(
                    "1203@g.us",
                    &format!("gif-demo-{index}"),
                    &path.to_string_lossy(),
                );
            }
            self.model.active = Some("1203@g.us".to_string());
            self.model.emoji_open = true;
            self.emoji_gif = true;
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
            } else if action == "favourite" {
                let favourited = self.model.toggle_favourite(&chat);
                self.notice = Some((
                    if favourited {
                        "Added to favourites".to_string()
                    } else {
                        "Removed from favourites".to_string()
                    },
                    std::time::Instant::now(),
                ));
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
                // Official order: favourite first, then the mutes and the rest.
                let favourite_label = if self.model.is_favourite(chat) {
                    "Remove from favourites"
                } else {
                    "Add to favourites"
                };
                if menu_row(ui, &palette, Icon::Star, favourite_label) {
                    chosen = Some("favourite".to_string());
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

    /// The poll composer: a question and three option rows, like the official
    /// dialog (more options can be added later; WhatsApp needs two).
    fn poll_window(&mut self, ctx: &egui::Context) {
        if self.poll_draft.is_none() {
            return;
        }
        let palette = self.palette.clone();
        let viewport = ctx.screen_rect();
        let mut send = false;
        let mut close = false;
        let draft = self.poll_draft.as_mut().expect("draft is open");
        dialog_card(
            ctx,
            "poll-dialog",
            360.0,
            "Create poll",
            &palette,
            viewport,
            |ui| {
                ui.label(
                    egui::RichText::new("QUESTION")
                        .size(11.5)
                        .color(palette.accent),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut draft.question)
                        .hint_text("Ask a question")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("OPTIONS")
                        .size(11.5)
                        .color(palette.accent),
                );
                for option in &mut draft.options {
                    ui.add(
                        egui::TextEdit::singleline(option)
                            .hint_text("Option")
                            .desired_width(f32::INFINITY),
                    );
                }
                ui.add_space(8.0);
                ui.checkbox(&mut draft.multiple, "Allow multiple answers");
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if dialog_button(ui, &palette, "Cancel", false).clicked() {
                        close = true;
                    }
                    if dialog_button(ui, &palette, "Send", true).clicked() {
                        send = true;
                    }
                });
            },
        );
        if send {
            let draft = self.poll_draft.clone().expect("draft is open");
            let options: Vec<String> = draft
                .options
                .into_iter()
                .map(|option| option.trim().to_string())
                .filter(|option| !option.is_empty())
                .collect();
            if let Some(chat) = self.model.active.clone() {
                if options.len() >= 2 && !draft.question.trim().is_empty() {
                    self.send(Command::SendPoll {
                        chat,
                        question: draft.question.trim().to_string(),
                        options,
                        multiple: draft.multiple,
                    });
                } else {
                    self.notice = Some((
                        "A poll needs a question and two options".to_string(),
                        std::time::Instant::now(),
                    ));
                }
            }
            self.poll_draft = None;
        } else if close {
            self.poll_draft = None;
        }
    }

    /// The contact-card composer: a name and a number.
    fn contact_window(&mut self, ctx: &egui::Context) {
        if self.contact_draft.is_none() {
            return;
        }
        let palette = self.palette.clone();
        let viewport = ctx.screen_rect();
        let mut send = false;
        let mut close = false;
        let draft = self.contact_draft.as_mut().expect("draft is open");
        dialog_card(
            ctx,
            "contact-dialog",
            340.0,
            "Share a contact",
            &palette,
            viewport,
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut draft.name)
                        .hint_text("Name")
                        .desired_width(f32::INFINITY),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut draft.phone)
                        .hint_text("Phone number")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if dialog_button(ui, &palette, "Cancel", false).clicked() {
                        close = true;
                    }
                    if dialog_button(ui, &palette, "Send", true).clicked() {
                        send = true;
                    }
                });
            },
        );
        if send {
            let draft = self.contact_draft.clone().expect("draft is open");
            if let Some(chat) = self.model.active.clone() {
                self.send(Command::SendContact {
                    chat,
                    name: draft.name.trim().to_string(),
                    phone: draft.phone.trim().to_string(),
                });
            }
            self.contact_draft = None;
        } else if close {
            self.contact_draft = None;
        }
    }

    /// The "New list" dialog: a name, and the open chat joins it.
    fn list_window(&mut self, ctx: &egui::Context) {
        if self.list_draft.is_none() {
            return;
        }
        let palette = self.palette.clone();
        let viewport = ctx.screen_rect();
        let mut create = false;
        let mut close = false;
        let draft = self.list_draft.as_mut().expect("draft is open");
        dialog_card(
            ctx,
            "list-dialog",
            320.0,
            "New list",
            &palette,
            viewport,
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(draft)
                        .hint_text("List name")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("The open chat joins the list; more can be added later.")
                        .size(12.0)
                        .color(palette.secondary),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if dialog_button(ui, &palette, "Cancel", false).clicked() {
                        close = true;
                    }
                    if dialog_button(ui, &palette, "Create", true).clicked() {
                        create = true;
                    }
                });
            },
        );
        if create {
            let name = self.list_draft.clone().unwrap_or_default();
            let chat = self.model.active.clone();
            match self.model.create_list(&name, chat.as_deref()) {
                Some(id) => {
                    self.model.chat_filter = crate::model::ChatFilter::List(id);
                }
                None => {
                    self.notice = Some((
                        "The list needs a name".to_string(),
                        std::time::Instant::now(),
                    ));
                }
            }
            self.list_draft = None;
        } else if close {
            self.list_draft = None;
        }
    }

    /// The location composer: a place name and the coordinates.
    fn location_window(&mut self, ctx: &egui::Context) {
        if self.location_draft.is_none() {
            return;
        }
        let palette = self.palette.clone();
        let viewport = ctx.screen_rect();
        let mut send = false;
        let mut close = false;
        let draft = self.location_draft.as_mut().expect("draft is open");
        dialog_card(
            ctx,
            "location-dialog",
            340.0,
            "Share a location",
            &palette,
            viewport,
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut draft.name)
                        .hint_text("Place name")
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.lat)
                            .hint_text("Latitude")
                            .desired_width(120.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.lng)
                            .hint_text("Longitude")
                            .desired_width(120.0),
                    );
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if dialog_button(ui, &palette, "Cancel", false).clicked() {
                        close = true;
                    }
                    if dialog_button(ui, &palette, "Send", true).clicked() {
                        send = true;
                    }
                });
            },
        );
        if send {
            let draft = self.location_draft.clone().expect("draft is open");
            let lat = draft.lat.trim().parse::<f64>().unwrap_or(0.0);
            let lng = draft.lng.trim().parse::<f64>().unwrap_or(0.0);
            if let Some(chat) = self.model.active.clone() {
                if lat != 0.0 || lng != 0.0 {
                    self.send(Command::SendLocation {
                        chat,
                        lat,
                        lng,
                        name: draft.name.trim().to_string(),
                    });
                } else {
                    self.notice = Some((
                        "Type a latitude and longitude".to_string(),
                        std::time::Instant::now(),
                    ));
                }
            }
            self.location_draft = None;
        } else if close {
            self.location_draft = None;
        }
    }

    /// The attach menu: the official entries (Photos & videos, Camera,
    /// Document, Audio, Poll, Location, Contact). The ones this client can send
    /// open the file picker; the rest say where they live.
    fn attach_window(&mut self, ctx: &egui::Context) {
        if !self.show_attach {
            return;
        }
        let palette = self.palette.clone();
        let mut chosen: Option<&'static str> = None;
        egui::Window::new("attach-menu")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(egui::pos2(
                ctx.screen_rect().left() + 74.0,
                ctx.screen_rect().bottom() - 470.0,
            ))
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
                    .inner_margin(egui::Margin::symmetric(3, 6)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(228.0);
                // Order sampled from the live client's attach menu on
                // 2026-11-05: Photos & videos, Camera, Document, Audio,
                // Contact, Poll. Location is this client's extra, kept last.
                for (icon, label, action) in [
                    (Icon::Document, "Photos & videos", "photos"),
                    (Icon::VideoCall, "Camera", "camera"),
                    (Icon::Document, "Document", "document"),
                    (Icon::Volume, "Audio", "audio"),
                    (Icon::PersonAdd, "Contact", "contact"),
                    (Icon::SelectChats, "Poll", "poll"),
                    (Icon::Status, "Location", "location"),
                ] {
                    if menu_row(ui, &palette, icon, label) {
                        chosen = Some(action);
                    }
                }
            });
        let Some(action) = chosen else { return };
        self.show_attach = false;
        match action {
            "photos" => self.pick_attachment(),
            "document" => self.pick_attachment(),
            "audio" => self.pick_attachment(),
            "camera" => {
                self.notice = Some((
                    "The camera capture is not in this build".to_string(),
                    std::time::Instant::now(),
                ));
            }
            "poll" => {
                self.poll_draft = Some(PollDraft {
                    question: String::new(),
                    options: [String::new(), String::new(), String::new()],
                    multiple: false,
                });
            }
            "contact" => {
                self.contact_draft = Some(ContactDraft {
                    name: String::new(),
                    phone: String::new(),
                });
            }
            "location" => {
                self.location_draft = Some(LocationDraft {
                    name: String::new(),
                    lat: String::new(),
                    lng: String::new(),
                });
            }
            _ => {}
        }
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

    /// Free the decoded frames and the worker of the open clip. Called when the
    /// viewer closes: a session that looked at ten videos should not keep all
    /// of their frames in memory.
    fn release_video(&mut self) {
        self.video_textures.clear();
        self.video_order.clear();
        self.video_prefetch = None;
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
    ///
    /// The paint thread never decodes: a worker keeps frames ready around the
    /// playhead ([`crate::frames::Prefetch`]) and this only uploads what has
    /// already been decoded. If the exact frame is still in flight the newest
    /// earlier frame is painted, so the video never blanks or flickers.
    fn video_frame_handle(&mut self, ctx: &egui::Context, id: &str) -> Option<egui::TextureHandle> {
        let VideoState::Ready(video) = self.videos.get(id)?.clone() else {
            return None;
        };
        let index = video.frame_at(self.video_elapsed());
        let dir_key = video.dir.display().to_string();

        // (Re)start the worker when another clip is open.
        let needs_worker = self
            .video_prefetch
            .as_ref()
            .is_none_or(|(key, _)| key != &dir_key);
        if needs_worker {
            self.video_prefetch = crate::frames::Prefetch::start(video.dir.clone(), video.frames)
                .map(|worker| (dir_key.clone(), worker));
        }
        if let Some((_, worker)) = &self.video_prefetch {
            worker.seek(index);
            let ready = worker.drain(4);
            for (number, image) in ready {
                let key = format!("{dir_key}#{number}");
                let texture = ctx.load_texture(key.clone(), image, egui::TextureOptions::LINEAR);
                self.video_textures.insert(key.clone(), Some(texture));
                self.video_order.push_back(key);
            }
        }
        // 18 frames at 640 px is ~0.6 s of look-ahead in video memory; the LRU
        // drops the frames behind the playhead as they are passed.
        while self.video_order.len() > 18 {
            if let Some(old) = self.video_order.pop_front() {
                self.video_textures.remove(&old);
            }
        }

        // The frame itself, otherwise the nearest one behind it.
        let key = format!("{dir_key}#{index}");
        if let Some(Some(texture)) = self.video_textures.get(&key) {
            return Some(texture.clone());
        }
        for behind in 1..=8 {
            if behind > index {
                break;
            }
            let key = format!("{dir_key}#{}", index - behind);
            if let Some(Some(texture)) = self.video_textures.get(&key) {
                return Some(texture.clone());
            }
        }
        None
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
        self.model.calls_outgoing = true;
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
                // A message that lands in the open chat is read straight away,
                // the way the official client behaves.
                if self.model.active.as_deref() == Some(chat.as_str()) {
                    self.send(Command::MarkRead { chat: chat.clone() });
                }
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
                Err(err) => {
                    diag(&format!("media {id} failed: {err}"));
                    self.model.media_failed_with(&id, &err);
                }
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
                    if rail_button(ui, &palette, Icon::Phone, self.show_calls, 0)
                        .on_hover_text("Calls")
                        .clicked()
                    {
                        self.show_calls = true;
                        self.show_status = false;
                        self.show_channels = false;
                        self.show_settings = false;
                        self.model.close_status();
                    }
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
                        self.show_calls = false;
                        self.show_communities = true;
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

        // Settings, Status and Channels replace the chat list, like the official
        // client: their own pane takes this column instead of stacking next to it.
        if self.show_settings
            || self.show_status
            || self.show_channels
            || self.show_calls
            || self.show_communities
        {
            return;
        }

        egui::SidePanel::left("chats")
            .resizable(true)
            .default_width(368.0)
            .show(ctx, |ui| {
                let palette = self.palette.clone();
                // The official list header is a 64 px bar in the rail colour
                // with the avatar and title inset by 12 px (measured on
                // web.whatsapp.com: header 64 px tall, padding 10/12).
                egui::Frame::new()
                    .fill(palette.elevated)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_min_height(44.0);
                        ui.horizontal(|ui| {
                            // The desktop header carries no avatar (it lives at
                            // the bottom of the rail) and a larger title.
                            ui.label(egui::RichText::new("Chats").size(26.0).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if fab_button(ui, &palette).on_hover_text("New chat").clicked()
                                    {
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
                                        ui.label(
                                            egui::RichText::new("⬆")
                                                .color(palette.accent)
                                                .size(16.0),
                                        )
                                        .on_hover_text(
                                            "An update is ready - see Settings → Updates",
                                        );
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(8.0);
                // The rounded search field.
                egui::Frame::new()
                    .fill(palette.raised)
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
                let (groups, communities, unread, favourites) = self.model.filter_counts();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    // The desktop client's order: All, Groups, Communities,
                    // Unread, then a caret for the rest (Favorites).
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
                    // Favorites hides behind the caret, as on the desktop.
                    if pill_button(ui, &palette, &format!("v {favourites}"), false) {
                        self.model.chat_filter = crate::model::ChatFilter::Favourites;
                    }
                    // Custom lists land here as their own pills, after the
                    // official "+ New list" control.
                    for list in self.model.lists.clone() {
                        let selected =
                            self.model.chat_filter == crate::model::ChatFilter::List(list.id);
                        if pill_button(ui, &palette, &list.name, selected) {
                            self.model.chat_filter = crate::model::ChatFilter::List(list.id);
                        }
                    }
                    if pill_button(ui, &palette, "+ New list", false) {
                        self.list_draft = Some(String::new());
                    }
                });
                ui.add_space(2.0);
                // The link line only shows when it is not simply "Connected":
                // the official list has no status row there.
                if !self.model.status.eq_ignore_ascii_case("connected") {
                    ui.label(
                        egui::RichText::new(&self.model.status)
                            .size(11.5)
                            .color(palette.secondary),
                    );
                }
                ui.add_space(2.0);

                let rows: Vec<ChatRow> = self.model.visible_chats().into_iter().cloned().collect();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    // Always reserve the bar: when it appeared mid-scroll the
                    // rows shifted a few pixels sideways, which flickers.
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        for row in &rows {
                            let selected = self.model.active.as_deref() == Some(row.jid.as_str());
                            let title = self.model.row_title(row);
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
                                crate::gif::clear();
                                crate::webp_anim::clear();
                                self.model.open_chat(&jid);
                                if jid.ends_with("@g.us") && self.model.request_group(&jid) {
                                    self.send(Command::GroupInfo { chat: jid.clone() });
                                }
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
                // The doodle wallpaper sits behind everything in the panel.
                paint_wallpaper(ui, &palette, ui.max_rect());
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
                                    egui::RichText::new(self.model.row_title(&row))
                                        .size(15.5)
                                        .strong(),
                                );
                                let subtitle = if row.jid.ends_with("@g.us") {
                                    match self.model.group.as_ref() {
                                        Some(info) if info.jid == row.jid => {
                                            wa_ui::labels::group_member_count(info.members.len())
                                        }
                                        _ => "Group".to_string(),
                                    }
                                } else if self.model.is_unsaved_person(&row.jid) {
                                    // An unsaved person's header line is the
                                    // number, like the official client.
                                    self.model
                                        .number_of(&row.jid)
                                        .unwrap_or_else(|| "online".to_string())
                                } else {
                                    "online".to_string()
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
                        matches!(
                            message.kind.as_str(),
                            "image" | "video" | "sticker" | "audio"
                        )
                    })
                    .take(24)
                    .map(|message| message.id.clone())
                    .collect();
                diag(&format!(
                    "chat {jid} messages {} media wanted {}",
                    row.messages.len(),
                    wanted.len()
                ));
                for id in wanted {
                    if self.model.request_media(&id) {
                        diag(&format!("request media {id}"));
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

                // The composer is painted *after* the list, so its real height
                // is measured for the next frame instead of guessed: a fixed
                // 64 px reserve clipped the field's bottom edge against the
                // window border as soon as the banner or the recording row made
                // the bar taller.
                let composer_height: f32 = ctx.data(|data| {
                    data.get_temp(egui::Id::new("composer-height"))
                        .unwrap_or(64.0)
                });
                let height = (ui.available_height() - composer_height - 10.0).max(120.0);
                // WhatsApp anchors a short conversation to the composer; the
                // previous frame's content height tells us how much to pad.
                // Keyed by chat: a *global* height padded the new chat with the
                // previous one's for a frame, so the list jumped on every click.
                let height_key = egui::Id::new(("chat-content-height", jid.as_str()));
                let content_height: f32 = ctx.data(|data| data.get_temp(height_key).unwrap_or(0.0));
                let spare = (height - content_height).max(0.0);
                let mut chosen: Option<BubbleAction> = None;
                let playback = self.audio.state();
                let mut list = egui::ScrollArea::vertical()
                    .stick_to_bottom(focus.is_none())
                    .auto_shrink([false, false])
                    .max_height(height.max(120.0));
                // A freshly opened chat (or a message we just sent) must be
                // visible, even when the history is far taller than the window:
                // `stick_to_bottom` alone only holds once the view is there, so
                // the content asks egui to scroll to its own bottom.
                let want_bottom = self.model.scroll_bottom > 0;
                if want_bottom {
                    self.model.scroll_bottom -= 1;
                    ui.ctx().request_repaint();
                }
                // `WA_DESK_SCROLL_TOP=1` parks the view at the first message so
                // the floating "newest message" disc can be checked.
                if self.preview_scroll_top {
                    list = list.vertical_scroll_offset(0.0);
                }
                let list_output = list.show(ui, |ui| {
                    let ctx = ui.ctx().clone();
                    // Breathing room from the window edge: the official
                    // list never lets a bubble touch the panel border. A
                    // frame margin insets *both* sides, which `add_space`
                    // cannot do for a right-aligned bubble.
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(20, 0))
                        .show(ui, |ui| {
                            let now = self.model.now;
                            for (index, message) in row.messages.iter().enumerate() {
                                // The official client separates days with a chip.
                                if Model::starts_new_day(&row.messages, index, now) {
                                    day_chip(
                                        ui,
                                        &palette,
                                        &wa_ui::labels::day_of(message.timestamp, now),
                                    );
                                }
                                // ...and marks where the unread block began.
                                if self
                                    .model
                                    .unread_anchor
                                    .get(&row.jid)
                                    .is_some_and(|id| id == &message.id)
                                {
                                    unread_divider(ui, &palette);
                                }
                                let is_hit = hits.contains(&index);
                                let head = Model::starts_run(&row.messages, index);
                                let avatar = if row.jid.ends_with("@g.us")
                                    && !message.from_me
                                    && head
                                    && let Some(sender) = message.sender.as_deref()
                                {
                                    self.ensure_avatar(ui.ctx(), sender);
                                    self.avatar_for(sender).cloned()
                                } else {
                                    None
                                };
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
                                    Model::ends_run(&row.messages, index),
                                    avatar.as_ref(),
                                    head,
                                ) {
                                    chosen = Some(action);
                                }
                            }
                            ctx.data_mut(|data| {
                                data.insert_temp(
                                    height_key,
                                    (ui.min_rect().height() - spare).max(0.0),
                                );
                            });
                        });
                    // Ask now that the content has a height; egui applies it at
                    // the end of this frame.
                    if want_bottom && !self.preview_scroll_top {
                        ui.scroll_to_rect(ui.min_rect(), Some(egui::Align::BOTTOM));
                    }
                });
                // The floating "jump to the newest message" disc the official
                // client shows once you scroll up in a long history: a 44 px
                // circle with a chevron, just above the composer.
                let viewport = list_output.inner_rect;
                let max_offset = (list_output.content_size.y - viewport.height()).max(0.0);
                if max_offset > 1.0 && list_output.state.offset.y < max_offset - 2.0 {
                    let center = egui::pos2(viewport.right() - 42.0, viewport.bottom() - 30.0);
                    let rect = egui::Rect::from_center_size(center, egui::vec2(44.0, 44.0));
                    let response = ui
                        .interact(
                            rect,
                            egui::Id::new("scroll-to-latest"),
                            egui::Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Scroll to the latest message");
                    let fill = if response.hovered() {
                        palette.hover
                    } else {
                        palette.elevated
                    };
                    let painter = ui.painter();
                    painter.circle_filled(center, 22.0, fill);
                    painter.circle_stroke(center, 21.5, egui::Stroke::new(1.0_f32, palette.border));
                    // The icon table has no "expand more", and two strokes stay
                    // crisp at any scale.
                    let chevron = egui::Stroke::new(2.0_f32, palette.text);
                    painter.line_segment(
                        [
                            center + egui::vec2(-6.0, -3.0),
                            center + egui::vec2(0.0, 3.0),
                        ],
                        chevron,
                    );
                    painter.line_segment(
                        [
                            center + egui::vec2(0.0, 3.0),
                            center + egui::vec2(6.0, -3.0),
                        ],
                        chevron,
                    );
                    if response.clicked() {
                        self.model.scroll_bottom = 3;
                        ui.ctx().request_repaint();
                    }
                }
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

                let composer_top = ui.cursor().top();
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
                            // Official composer: attach on the left, then the
                            // rounded field with emoji + stickers inside it on
                            // the right, then the green send button.
                            if icon_button(ui, Icon::Attach, &palette)
                                .on_hover_text("Attach")
                                .clicked()
                            {
                                self.show_attach = !self.show_attach;
                            }
                            let send_width = 46.0;
                            let input_width = ui.available_width() - send_width;
                            let field_width = (input_width - 98.0).max(80.0);
                            // The official field is 38 px tall inside a 62 px bar.
                            let field_height = 38.0;
                            let mut edit_response = None;
                            // The single-line field consumes the Enter event
                            // (`consume_key`) while it surrenders focus, so the
                            // key has to be sampled before the widget is drawn.
                            let enter_pressed =
                                ui.input(|input| input.key_pressed(egui::Key::Enter));
                            egui::Frame::new()
                                .fill(palette.raised)
                                .corner_radius(egui::CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(12, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        edit_response = Some(
                                            ui.add_sized(
                                                [field_width, field_height],
                                                egui::TextEdit::singleline(
                                                    &mut self.model.composer,
                                                )
                                                .frame(false)
                                                .hint_text("Type a message"),
                                            ),
                                        );
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
                                    });
                                });
                            let edit = edit_response.expect("the composer field is created above");
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
                            // `has_focus` covers the frame before the field
                            // surrenders focus, `lost_focus` the one after it.
                            let enter =
                                enter_sends(enter_pressed, edit.has_focus() || edit.lost_focus());
                            // The official rule: an empty draft shows the
                            // microphone, a typed one shows send.
                            let empty_draft = self.model.composer.trim().is_empty()
                                && self.model.reply_to.is_none()
                                && self.model.editing.is_none();
                            if empty_draft
                                && self.voice.is_none()
                                && icon_button(ui, Icon::Mic, &palette)
                                    .on_hover_text("Record a voice note")
                                    .clicked()
                            {
                                match crate::voicenote::Recording::start() {
                                    Ok(recording) => self.voice = Some(recording),
                                    Err(err) => {
                                        self.notice = Some((err, std::time::Instant::now()))
                                    }
                                }
                            }
                            if let Some(recording) = self.voice.as_ref() {
                                // While recording: the elapsed time, a cancel
                                // button and the send button.
                                let seconds = recording.seconds();
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{:0}:{:02}",
                                        (seconds as u32) / 60,
                                        (seconds as u32) % 60
                                    ))
                                    .size(13.0)
                                    .color(palette.danger),
                                );
                                if icon_button(ui, Icon::Close, &palette)
                                    .on_hover_text("Cancel the recording")
                                    .clicked()
                                {
                                    self.voice = None;
                                }
                                if icon_button(ui, Icon::Send, &palette)
                                    .on_hover_text("Send the voice note")
                                    .clicked()
                                    && let Some(recording) = self.voice.take()
                                {
                                    match recording.finish() {
                                        Ok(note) => {
                                            use base64::Engine as _;
                                            let payload = base64::engine::general_purpose::STANDARD
                                                .encode(&note.ogg);
                                            self.send(Command::SendVoiceNote {
                                                chat: jid.clone(),
                                                payload,
                                                seconds: note.seconds,
                                                waveform: note.waveform,
                                            });
                                        }
                                        Err(err) => {
                                            self.notice = Some((err, std::time::Instant::now()))
                                        }
                                    }
                                }
                            } else if icon_button(ui, Icon::Send, &palette)
                                .on_hover_text(self.model.send_label())
                                .clicked()
                                || (enter && !empty_draft)
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
                                        // WhatsApp does not echo our own
                                        // messages back, so the bubble has to
                                        // exist before the stanza leaves.
                                        self.model.add_own_text(&jid, &text, Some(&reply));
                                        self.send(Command::SendReply {
                                            chat: jid.clone(),
                                            text,
                                            reply_id: reply.id,
                                            quoted,
                                        });
                                    } else {
                                        self.model.add_own_text(&jid, &text, None);
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
                        ctx.data_mut(|data| {
                            data.insert_temp(
                                egui::Id::new("composer-height"),
                                (ui.min_rect().bottom() - composer_top).max(0.0),
                            );
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
            | BubbleAction::LoadMedia(id)
            | BubbleAction::Forward(id) => id.clone(),
            BubbleAction::React(id, _) => id.clone(),
        };
        let message = row.messages.iter().find(|item| item.id == id).cloned();
        match action {
            BubbleAction::OpenMedia(id) => {
                self.model.open_viewer(&id);
            }
            BubbleAction::LoadMedia(id) => {
                self.model.retry_media(&id);
                if self.model.request_media(&id) {
                    self.send(Command::FetchMedia { id });
                }
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

/// Decode the inline thumbnail (a data URI) into a texture, cached under
/// thumb-<id>. Used only while the real file is still downloading, so the
/// bubble is never an empty placeholder.
pub(crate) fn thumb_texture(
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<String, Option<egui::TextureHandle>>,
    id: &str,
    thumb: Option<&str>,
) -> Option<egui::TextureHandle> {
    use base64::Engine as _;
    let data = thumb?;
    let key = format!("thumb-{id}");
    if let Some(texture) = cache.get(&key) {
        return texture.clone();
    }
    let encoded = data.split_once(',').map(|(_, body)| body).unwrap_or(data);
    let texture = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()
        .and_then(|bytes| image::load_from_memory(&bytes).ok())
        .map(|image| {
            let image = image.to_rgba8();
            let size = [image.width() as usize, image.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
            ctx.load_texture(key.clone(), color, egui::TextureOptions::LINEAR)
        });
    cache.insert(key, texture.clone());
    texture
}

/// Enter sends the message only while the composer has (or has just lost) the
/// keyboard focus: without that guard a stray Enter anywhere in the window -
/// the search field, a dialog - would post the draft.
fn enter_sends(enter_pressed: bool, composer_focused: bool) -> bool {
    enter_pressed && composer_focused
}

/// Keep the texture caches bounded: drop the least-recently-painted entries
/// once a cache grows past `cap`. This is what stops a long session from
/// holding every photo and sticker it ever scrolled past in video memory.
pub(crate) fn prune_textures(
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<String, Option<egui::TextureHandle>>,
    cap: usize,
) {
    if cache.len() <= cap {
        return;
    }
    let now = ctx.cumulative_frame_nr();
    let mut used: Vec<(u64, String)> = cache
        .keys()
        .map(|key| {
            let stamp = ctx
                .data(|data| data.get_temp::<u64>(egui::Id::new(("texture-use", key.as_str()))))
                .unwrap_or(0);
            (stamp, key.clone())
        })
        .collect();
    used.sort_unstable();
    // Everything painted this frame stays; the rest goes oldest first.
    let mut live = used.len();
    let kept: Vec<(u64, String)> = used;
    for (stamp, key) in &kept {
        if live <= cap {
            break;
        }
        if *stamp == now {
            continue;
        }
        cache.remove(key);
        live -= 1;
    }
    // A single frame that painted more than `cap` textures is still capped:
    // those extra ones simply re-decode next frame.
    for (_, key) in &kept {
        if cache.len() <= cap {
            break;
        }
        cache.remove(key);
    }
}

/// A laid-out label with every occurrence of the query on a yellow band.
pub(crate) fn highlight_job(
    text: &str,
    needle: &str,
    color: egui::Color32,
) -> egui::text::LayoutJob {
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
    /// The login gate: WhatsApp Web's "scan to log in" page, painted with the
    /// dark palette (wordmark, download card, the scan card with its three
    /// numbered steps, the QR and the phone-number link).
    fn gate(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(self.palette.panel))
            .show(ctx, |ui| {
                let palette = self.palette.clone();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Top-left wordmark, like the web page.
                        ui.add_space(18.0);
                        ui.horizontal(|ui| {
                            ui.add_space(26.0);
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::hover());
                            draw_whatsapp_mark(ui, rect, palette.accent);
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("WhatsApp")
                                    .size(15.0)
                                    .color(palette.text)
                                    .strong(),
                            );
                        });
                        ui.add_space(46.0);

                        let width = (ui.available_width() - 96.0).clamp(420.0, 860.0);
                        let mut go_phone = false;
                        let mut new_code = false;

                        ui.vertical_centered(|ui| {
                            // ---- Download card -------------------------------------
                            gate_card(ui, &palette, width, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(120.0, 84.0),
                                        egui::Sense::hover(),
                                    );
                                    draw_laptop(ui, rect, &palette);
                                    ui.add_space(18.0);
                                    ui.vertical(|ui| {
                                        ui.label(
                                            egui::RichText::new("Download WA Desk for Windows")
                                                .size(17.0)
                                                .color(palette.text)
                                                .strong(),
                                        );
                                        ui.add_space(3.0);
                                        ui.label(
                                            egui::RichText::new(
                                                "Get extra features like voice and video calling, \
                                                 screen sharing and more.",
                                            )
                                            .size(13.5)
                                            .color(palette.secondary),
                                        );
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let (rect, response) = ui.allocate_exact_size(
                                                egui::vec2(148.0, 44.0),
                                                egui::Sense::click(),
                                            );
                                            let fill = if response.hovered() {
                                                palette.accent.gamma_multiply(0.88)
                                            } else {
                                                palette.accent
                                            };
                                            ui.painter().rect_filled(
                                                rect,
                                                egui::CornerRadius::same(24),
                                                fill,
                                            );
                                            let label = ui.painter().layout_no_wrap(
                                                "Download".to_string(),
                                                egui::FontId::proportional(15.0),
                                                egui::Color32::from_rgb(11, 20, 26),
                                            );
                                            let icon = egui::Rect::from_center_size(
                                                egui::pos2(
                                                    rect.right() - 26.0,
                                                    rect.center().y,
                                                ),
                                                egui::vec2(18.0, 18.0),
                                            );
                                            ui.painter().galley(
                                                egui::pos2(
                                                    rect.left() + 22.0,
                                                    rect.center().y - label.size().y / 2.0,
                                                ),
                                                label,
                                                egui::Color32::from_rgb(11, 20, 26),
                                            );
                                            draw_icon(
                                                ui,
                                                icon,
                                                Icon::Download,
                                                egui::Color32::from_rgb(11, 20, 26),
                                            );
                                            if response.clicked() {
                                                self.notice = Some((
                                                    "The installer is the WA-Desk-Setup.exe \
                                                     release download"
                                                        .to_string(),
                                                    std::time::Instant::now(),
                                                ));
                                            }
                                        },
                                    );
                                });
                            });

                            ui.add_space(18.0);

                            // ---- Scan card -----------------------------------------
                            gate_card(ui, &palette, width, |ui| {
                                ui.horizontal_top(|ui| {
                                    // Left: title, steps, links.
                                    ui.vertical(|ui| {
                                        ui.set_width((width * 0.5 - 60.0).max(240.0));
                                        ui.label(
                                            egui::RichText::new("Scan to log in")
                                                .size(27.0)
                                                .color(palette.text),
                                        );
                                        ui.add_space(16.0);
                                        gate_step(
                                            ui,
                                            &palette,
                                            1,
                                            |ui, palette| {
                                                ui.label(
                                                    egui::RichText::new(
                                                        "Scan the QR code with your phone's camera",
                                                    )
                                                    .size(14.0)
                                                    .color(palette.text),
                                                );
                                            },
                                        );
                                        ui.add_space(10.0);
                                        gate_step(
                                            ui,
                                            &palette,
                                            2,
                                            |ui, palette| {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(
                                                            "Tap the link to open WhatsApp",
                                                        )
                                                        .size(14.0)
                                                        .color(palette.text),
                                                    );
                                                    let (rect, _) = ui.allocate_exact_size(
                                                        egui::vec2(15.0, 15.0),
                                                        egui::Sense::hover(),
                                                    );
                                                    draw_whatsapp_mark(ui, rect, palette.accent);
                                                });
                                            },
                                        );
                                        ui.add_space(10.0);
                                        gate_step(
                                            ui,
                                            &palette,
                                            3,
                                            |ui, palette| {
                                                ui.label(
                                                    egui::RichText::new(
                                                        "Scan the QR code again to link your account",
                                                    )
                                                    .size(14.0)
                                                    .color(palette.text),
                                                );
                                            },
                                        );
                                        ui.add_space(14.0);
                                        if gate_link(ui, &palette, "Need help?")
                                            .on_hover_text(
                                                "Open WhatsApp on your phone, then Linked devices \
                                                 → Link a device.",
                                            )
                                            .clicked()
                                        {
                                            self.notice = Some((
                                                "WhatsApp on your phone → Linked devices → \
                                                 Link a device"
                                                    .to_string(),
                                                std::time::Instant::now(),
                                            ));
                                        }
                                    });

                                    // Right: the QR (or the pairing code) and the links.
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Min),
                                        |ui| {
                                            ui.vertical(|ui| {
                                                let side = 268.0;
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Min),
                                                    |ui| {
                                                        ui.allocate_ui(
                                                            egui::vec2(side + 24.0, side + 24.0),
                                                            |ui| {
                                                                let (rect, _) =
                                                                    ui.allocate_exact_size(
                                                                        egui::vec2(
                                                                            side + 24.0,
                                                                            side + 24.0,
                                                                        ),
                                                                        egui::Sense::hover(),
                                                                    );
                                                                ui.painter().rect_filled(
                                                                    rect,
                                                                    egui::CornerRadius::same(16),
                                                                    egui::Color32::WHITE,
                                                                );
                                                                let qr_rect = rect.shrink(12.0);
                                                                match self.model.login.clone() {
                                                                    LoginState::Qr(svg) => {
                                                                        if let Some(texture) =
                                                                            self.qr_texture(ctx, &svg)
                                                                        {
                                                                            ui.painter().image(
                                                                                texture.id(),
                                                                                qr_rect,
                                                                                egui::Rect::from_min_max(
                                                                                    egui::pos2(0.0, 0.0),
                                                                                    egui::pos2(1.0, 1.0),
                                                                                ),
                                                                                egui::Color32::WHITE,
                                                                            );
                                                                        }
                                                                    }
                                                                    LoginState::PairCode(code) => {
                                                                        let galley = ui
                                                                            .painter()
                                                                            .layout_no_wrap(
                                                                                code,
                                                                                egui::FontId::monospace(26.0),
                                                                                egui::Color32::from_rgb(17, 27, 33),
                                                                            );
                                                                        ui.painter().galley(
                                                                            egui::pos2(
                                                                                rect.center().x
                                                                                    - galley.size().x
                                                                                        / 2.0,
                                                                                rect.center().y
                                                                                    - galley.size().y
                                                                                        / 2.0,
                                                                            ),
                                                                            galley,
                                                                            egui::Color32::from_rgb(
                                                                                17, 27, 33,
                                                                            ),
                                                                        );
                                                                    }
                                                                    LoginState::Expired
                                                                    | LoginState::Waiting
                                                                    | LoginState::Linked => {}
                                                                }
                                                            },
                                                        );
                                                    },
                                                );
                                                ui.add_space(14.0);
                                                if self.gate_phone {
                                                    ui.vertical_centered(|ui| {
                                                        let width =
                                                            (ui.available_width()).min(240.0);
                                                        ui.add_sized(
                                                            [width, 30.0],
                                                            egui::TextEdit::singleline(
                                                                &mut self.model.pair_phone,
                                                            )
                                                            .hint_text("628123456789"),
                                                        );
                                                        ui.add_space(6.0);
                                                        let (rect, response) =
                                                            ui.allocate_exact_size(
                                                                egui::vec2(120.0, 30.0),
                                                                egui::Sense::click(),
                                                            );
                                                        ui.painter().rect_filled(
                                                            rect,
                                                            egui::CornerRadius::same(24),
                                                            palette.accent,
                                                        );
                                                        ui.painter().text(
                                                            rect.center(),
                                                            egui::Align2::CENTER_CENTER,
                                                            "Get code",
                                                            egui::FontId::proportional(13.5),
                                                            egui::Color32::from_rgb(11, 20, 26),
                                                        );
                                                        if response.clicked() {
                                                            let phone: String = self
                                                                .model
                                                                .pair_phone
                                                                .chars()
                                                                .filter(|c| c.is_ascii_digit())
                                                                .collect();
                                                            if !phone.is_empty() {
                                                                self.send(Command::PairCode {
                                                                    phone,
                                                                });
                                                            }
                                                        }
                                                    });
                                                } else {
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            if gate_link(
                                                                ui,
                                                                &palette,
                                                                "Log in with phone number",
                                                            )
                                                            .clicked()
                                                            {
                                                                go_phone = true;
                                                            }
                                                            draw_icon(
                                                                ui,
                                                                egui::Rect::from_center_size(
                                                                    egui::pos2(
                                                                        ui.cursor().left() + 8.0,
                                                                        ui.cursor().top() + 8.0,
                                                                    ),
                                                                    egui::vec2(16.0, 16.0),
                                                                ),
                                                                Icon::ChevronRight,
                                                                palette.accent,
                                                            );
                                                        },
                                                    );
                                                }
                                            });
                                        },
                                    );
                                });

                                ui.add_space(16.0);
                                // The session is stored on disk, so this is a
                                // truthful checked box rather than a switch.
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui
                                        .allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                                    ui.painter().rect_filled(
                                        rect,
                                        egui::CornerRadius::same(4),
                                        palette.accent,
                                    );
                                    draw_icon(
                                        ui,
                                        rect.shrink(3.0),
                                        Icon::Check,
                                        egui::Color32::from_rgb(11, 20, 26),
                                    );
                                    ui.add_space(6.0);
                                    ui.label(
                                        egui::RichText::new("Stay logged in on this device")
                                            .size(13.5)
                                            .color(palette.text),
                                    );
                                    draw_icon(
                                        ui,
                                        egui::Rect::from_min_size(
                                            egui::pos2(ui.cursor().left() + 6.0, ui.cursor().top() + 2.0),
                                            egui::vec2(15.0, 15.0),
                                        ),
                                        Icon::Lock,
                                        palette.secondary,
                                    );
                                    ui.add_space(21.0);
                                    if crate::login::can_restart(&self.model.login)
                                        && gate_link(ui, &palette, "Request a new code").clicked()
                                    {
                                        new_code = true;
                                    }
                                });
                            });

                            ui.add_space(22.0);
                            ui.vertical_centered(|ui| {
                                ui.horizontal(|ui| {
                                    let text = ui.painter().layout_no_wrap(
                                        "Don't have a WhatsApp account?".to_string(),
                                        egui::FontId::proportional(14.0),
                                        palette.text,
                                    );
                                    let link = ui.painter().layout_no_wrap(
                                        "Get started".to_string(),
                                        egui::FontId::proportional(14.0),
                                        palette.accent,
                                    );
                                    let total = text.size().x + 6.0 + link.size().x + 16.0;
                                    ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                                    ui.label(
                                        egui::RichText::new("Don't have a WhatsApp account?")
                                            .size(14.0)
                                            .color(palette.text),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new("Get started")
                                            .size(14.0)
                                            .color(palette.accent)
                                            .underline(),
                                    );
                                });
                                ui.add_space(14.0);
                                ui.horizontal(|ui| {
                                    let galley = ui.painter().layout_no_wrap(
                                        "Your personal messages are end-to-end encrypted"
                                            .to_string(),
                                        egui::FontId::proportional(13.0),
                                        palette.secondary,
                                    );
                                    let total = galley.size().x + 22.0;
                                    ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(15.0, 15.0),
                                        egui::Sense::hover(),
                                    );
                                    draw_icon(ui, rect, Icon::Lock, palette.secondary);
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "Your personal messages are end-to-end encrypted",
                                        )
                                        .size(13.0)
                                        .color(palette.secondary),
                                    );
                                });
                                ui.add_space(12.0);
                                ui.label(
                                    egui::RichText::new("Terms & Privacy Policy")
                                        .size(12.0)
                                        .color(palette.secondary.gamma_multiply(0.8)),
                                );
                            });
                            ui.add_space(28.0);
                        });

                        if go_phone {
                            self.gate_phone = true;
                        }
                        if new_code {
                            self.model.login_restarted();
                            self.send(Command::Restart);
                        }
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
    /// The Communities screen: the official empty state ("Create communities")
    /// or the list of communities with their groups, announcements first.
    fn communities_panel(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_min_height(40.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Communities")
                                    .size(20.0)
                                    .color(palette.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(ui, Icon::Close, &palette)
                                        .on_hover_text("Back to chats")
                                        .clicked()
                                    {
                                        self.show_communities = false;
                                    }
                                },
                            );
                        });
                    });

                let communities = self
                    .model
                    .communities()
                    .into_iter()
                    .map(|(parent, children)| {
                        (
                            parent.clone(),
                            children.into_iter().cloned().collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>();

                if communities.is_empty() {
                    ui.add_space(18.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Create communities")
                                    .size(17.0)
                                    .color(palette.text),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(
                                    "Bring members together in topic-based groups and easily send \
                                     them admin announcements. Your personal messages in \
                                     communities are end-to-end encrypted",
                                )
                                .size(13.0)
                                .color(palette.secondary),
                            );
                            ui.add_space(10.0);
                            if status_share_tile(ui, &palette, Icon::Groups, "New community") {
                                self.notice = Some((
                                    "Creating a community needs the phone".to_string(),
                                    std::time::Instant::now(),
                                ));
                            }
                        });
                    });
                    return;
                }

                // The official screen opens the first community, so its groups
                // (announcements first) are visible without a click.
                if self.communities_active.is_none() {
                    self.communities_active = communities.first().map(|(p, _)| p.jid.clone());
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(8.0);
                        for (parent, children) in &communities {
                            self.ensure_avatar(ui.ctx(), &parent.jid);
                            let avatar = self.avatar_for(&parent.jid).cloned();
                            let selected =
                                self.communities_active.as_deref() == Some(parent.jid.as_str());
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 72.0),
                                egui::Sense::click(),
                            );
                            let card = rect.shrink2(egui::vec2(8.0, 0.0));
                            if selected || response.hovered() {
                                ui.painter().rect_filled(
                                    card,
                                    egui::CornerRadius::same(12),
                                    if selected {
                                        palette.selected
                                    } else {
                                        palette.hover
                                    },
                                );
                            }
                            // Painted (not allocated): allocating inside the row
                            // pushed the avatar onto the next line.
                            let centre = egui::pos2(rect.left() + 44.0, rect.center().y);
                            if let Some(texture) = avatar.as_ref() {
                                ui.painter().image(
                                    texture.id(),
                                    egui::Rect::from_center_size(centre, egui::vec2(48.0, 48.0)),
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            } else {
                                ui.painter().circle_filled(
                                    centre,
                                    24.0,
                                    avatar_color(&parent.name),
                                );
                                ui.painter().text(
                                    centre,
                                    egui::Align2::CENTER_CENTER,
                                    initials(&parent.name),
                                    egui::FontId::proportional(16.0),
                                    egui::Color32::WHITE,
                                );
                            }
                            ui.painter().text(
                                egui::pos2(rect.left() + 82.0, rect.center().y - 9.0),
                                egui::Align2::LEFT_CENTER,
                                fit_text(
                                    ui.painter(),
                                    &parent.name,
                                    egui::FontId::proportional(15.0),
                                    palette.text,
                                    rect.width() - 100.0,
                                ),
                                egui::FontId::proportional(15.0),
                                palette.text,
                            );
                            let count = children.len();
                            ui.painter().text(
                                egui::pos2(rect.left() + 82.0, rect.center().y + 11.0),
                                egui::Align2::LEFT_CENTER,
                                if count == 1 {
                                    "1 group".to_string()
                                } else {
                                    format!("{count} groups")
                                },
                                egui::FontId::proportional(12.5),
                                palette.secondary,
                            );
                            if response.clicked() {
                                self.communities_active = Some(parent.jid.clone());
                                if self.model.request_group(&parent.jid) {
                                    self.send(Command::GroupInfo {
                                        chat: parent.jid.clone(),
                                    });
                                }
                            }

                            // The selected community opens its groups below it.
                            if selected {
                                ui.add_space(2.0);
                                for child in children {
                                    let (row, hit) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 56.0),
                                        egui::Sense::click(),
                                    );
                                    if hit.hovered() {
                                        ui.painter().rect_filled(
                                            row.shrink2(egui::vec2(8.0, 0.0)),
                                            egui::CornerRadius::same(12),
                                            palette.hover,
                                        );
                                    }
                                    self.ensure_avatar(ui.ctx(), &child.jid);
                                    let avatar = self.avatar_for(&child.jid).cloned();
                                    let dot = egui::pos2(row.left() + 26.0, row.center().y);
                                    if let Some(texture) = avatar.as_ref() {
                                        ui.painter().image(
                                            texture.id(),
                                            egui::Rect::from_center_size(
                                                dot,
                                                egui::vec2(36.0, 36.0),
                                            ),
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            egui::Color32::WHITE,
                                        );
                                    } else {
                                        ui.painter().circle_filled(
                                            dot,
                                            18.0,
                                            avatar_color(&child.name),
                                        );
                                        ui.painter().text(
                                            dot,
                                            egui::Align2::CENTER_CENTER,
                                            initials(&child.name),
                                            egui::FontId::proportional(13.0),
                                            egui::Color32::WHITE,
                                        );
                                    }
                                    ui.painter().text(
                                        egui::pos2(row.left() + 70.0, row.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        fit_text(
                                            ui.painter(),
                                            &child.name,
                                            egui::FontId::proportional(14.5),
                                            palette.text,
                                            row.width() - 90.0,
                                        ),
                                        egui::FontId::proportional(14.5),
                                        palette.text,
                                    );
                                    if child.announce {
                                        ui.painter().text(
                                            egui::pos2(row.right() - 22.0, row.center().y),
                                            egui::Align2::RIGHT_CENTER,
                                            "Announcements",
                                            egui::FontId::proportional(11.5),
                                            palette.secondary,
                                        );
                                    }
                                    if hit.clicked() {
                                        let jid = child.jid.clone();
                                        self.show_communities = false;
                                        self.model.open_chat(&jid);
                                    }
                                }
                            }
                        }
                    });
            });
    }

    /// The Calls screen: the 64 px header, then the call log as 72 px rows with
    /// the direction glyph, the date and a call button - the official layout.
    fn calls_panel(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_min_height(40.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Calls").size(20.0).color(palette.text));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(ui, Icon::Close, &palette)
                                        .on_hover_text("Back to chats")
                                        .clicked()
                                    {
                                        self.show_calls = false;
                                    }
                                },
                            );
                        });
                    });

                if self.model.call_log.is_empty() {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("No calls yet")
                                .size(14.0)
                                .color(palette.secondary),
                        );
                    });
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let entries = self.model.call_log.clone();
                        for entry in entries {
                            self.ensure_avatar(ui.ctx(), &entry.jid);
                            let avatar = self.avatar_for(&entry.jid).cloned();
                            if let Some(call) = call_row(ui, &palette, &entry, avatar.as_ref()) {
                                // Start a call with that person from the list.
                                self.model.open_chat(&entry.jid);
                                self.model.calls_outgoing = true;
                                self.send(Command::CallStart {
                                    chat: entry.jid.clone(),
                                    video: call,
                                });
                            }
                        }
                    });
            });
    }

    /// The Status screen: the official list on the left (a 64 px header, the
    /// "My status" row, then Recent/Viewed) and the "Share statuses" pane on the
    /// right with the Text / Photo-and-video tiles.
    fn status_panel(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        if self.model.viewer.is_some() {
            egui::CentralPanel::default().show(ctx, |ui| {
                self.status_viewer(ui, ctx);
            });
            return;
        }

        // Right: the share pane, 320 px, the official empty-state card.
        egui::SidePanel::right("status-share")
            .resizable(false)
            .exact_width(320.0)
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Share statuses")
                                .size(20.0)
                                .color(palette.text),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(
                                "Share photos, videos and text that disappear after 24 hours.",
                            )
                            .size(13.5)
                            .color(palette.secondary),
                        );
                        ui.add_space(18.0);
                        // Two tiles: text first, then photo/video, as official.
                        if status_share_tile(ui, &palette, Icon::Edit, "Text status") {
                            self.status_compose = Some(String::new());
                        }
                        ui.add_space(10.0);
                        if status_share_tile(ui, &palette, Icon::Document, "Photo and video")
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Photo or video", &["jpg", "jpeg", "png", "mp4"])
                                .pick_file()
                        {
                            self.send(Command::PostStatusMedia {
                                path: path.to_string_lossy().into_owned(),
                                caption: String::new(),
                            });
                        }
                        if self.status_compose.is_some() {
                            ui.add_space(16.0);
                            // The official composer row: Emoji, Font, Palette,
                            // Send, and a close control.
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Text status")
                                        .size(13.0)
                                        .color(palette.accent),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if icon_button(ui, Icon::Close, &palette)
                                            .on_hover_text("Close status posting view")
                                            .clicked()
                                        {
                                            self.status_compose = None;
                                        }
                                        if icon_button(ui, Icon::Emoji, &palette)
                                            .on_hover_text("Emoji")
                                            .clicked()
                                        {
                                            self.model.emoji_open = !self.model.emoji_open;
                                        }
                                    },
                                );
                            });
                            // The box takes the chosen background, like the
                            // official composer, and the palette below is the
                            // official 20-colour array from `wa-ui`.
                            let swatches = wa_ui::composer::palette();
                            let background = swatches
                                .backgrounds
                                .get(self.status_compose_background)
                                .cloned()
                                .unwrap_or_else(|| "#A62C71".to_string());
                            let fill = hex_colour(&background);
                            let mut draft = self.status_compose.clone().unwrap_or_default();
                            egui::Frame::new()
                                .fill(fill)
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    ui.add_sized(
                                        [240.0, 64.0],
                                        egui::TextEdit::multiline(&mut draft)
                                            .frame(false)
                                            .hint_text("Type a status"),
                                    );
                                });
                            self.status_compose = Some(draft.clone());
                            ui.add_space(8.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for (index, hex) in swatches.backgrounds.iter().enumerate() {
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(20.0, 20.0),
                                        egui::Sense::click(),
                                    );
                                    ui.painter().circle_filled(
                                        rect.center(),
                                        10.0,
                                        hex_colour(hex),
                                    );
                                    if index == self.status_compose_background {
                                        ui.painter().circle_stroke(
                                            rect.center(),
                                            9.0,
                                            egui::Stroke::new(2.0_f32, palette.text),
                                        );
                                    }
                                    if response.clicked() {
                                        self.status_compose_background = index;
                                    }
                                }
                            });
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                for (index, name) in swatches.font_names.iter().enumerate() {
                                    let selected = index == self.status_compose_font;
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new(name).size(12.5).color(
                                                    if selected {
                                                        palette.accent
                                                    } else {
                                                        palette.secondary
                                                    },
                                                ),
                                            )
                                            .frame(false),
                                        )
                                        .clicked()
                                    {
                                        self.status_compose_font = index;
                                    }
                                }
                            });
                            ui.add_space(8.0);
                            if status_send_button(ui, &palette) && !draft.trim().is_empty() {
                                let font = swatches
                                    .fonts
                                    .get(self.status_compose_font)
                                    .cloned()
                                    .unwrap_or_else(|| "system".to_string());
                                self.send(Command::PostStatusText {
                                    text: draft.trim().to_string(),
                                    background: wa_ui::composer::argb(&background),
                                    font,
                                });
                                self.status_compose = None;
                            }
                        }
                    });
                });
            });

        // Left: the list.
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_min_height(40.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Status").size(20.0).color(palette.text));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(ui, Icon::Close, &palette)
                                        .on_hover_text("Back to chats")
                                        .clicked()
                                    {
                                        self.show_status = false;
                                    }
                                    if icon_button(ui, Icon::Edit, &palette)
                                        .on_hover_text("Text status")
                                        .clicked()
                                    {
                                        self.status_compose = Some(String::new());
                                    }
                                },
                            );
                        });
                    });

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // My status: the own avatar with a dashed "add" ring.
                        let own = self.model.own_name();
                        self.ensure_avatar(ui.ctx(), &self.model.me_jid.clone());
                        let avatar = self.avatar_for(&self.model.me_jid.clone()).cloned();
                        if status_row(ui, &palette, &own, "", avatar.as_ref(), false, true) {
                            self.status_compose = Some(String::new());
                        }
                        let (recent, viewed) = self.model.status_sections();
                        if recent.is_empty() && viewed.is_empty() {
                            ui.add_space(26.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("No recent updates")
                                        .size(14.0)
                                        .color(palette.secondary),
                                );
                            });
                            return;
                        }
                        for (heading, groups) in [("Recent", &recent), ("Viewed", &viewed)] {
                            if groups.is_empty() {
                                continue;
                            }
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                ui.label(
                                    egui::RichText::new(heading)
                                        .size(13.0)
                                        .color(palette.accent),
                                );
                            });
                            ui.add_space(2.0);
                            for group in groups.iter() {
                                self.ensure_avatar(ui.ctx(), &group.jid);
                                let avatar = self.avatar_for(&group.jid).cloned();
                                let stamp = format!("{} update(s)", group.count);
                                if status_row(
                                    ui,
                                    &palette,
                                    &group.name,
                                    &stamp,
                                    avatar.as_ref(),
                                    group.unseen,
                                    false,
                                ) {
                                    let jid = group.jid.clone();
                                    self.model.open_status(&jid);
                                }
                            }
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

        // The live viewer stacks the author over the timestamp, top-left
        // (sampled 2026-11-05: "Daffa" / "Yesterday at 10:29 PM") instead of
        // running them together on one line.
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&update.name).size(14.5).strong());
                ui.label(
                    egui::RichText::new(self.model.bubble_stamp(&status_as_message(&update)))
                        .size(12.5)
                        .color(egui::Color32::from_gray(170)),
                );
            });
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
        gif: false,
        status_quote: false,
        contact: None,
        poll: None,
        location: None,
        thumb: None,
    }
}

/// Fallback faces the bundled set lacks, in fallback order: emoji, Chinese
/// (simplified *and* traditional), Japanese, then Korean. Before this, non-Latin
/// text drew as empty boxes.
/// Base-OS faces for symbol text that the Latin, emoji and CJK faces do not
/// carry: Mathematical Alphanumeric Symbols (U+1D400-U+1D7FF, e.g. "𝕋𝕒𝕦𝕗𝕚𝕢"),
/// arrows, box drawing, dingbat *letters* (U+2701-U+27BE without emoji
/// presentation). Segoe UI Symbol ships with stock Windows; the Linux and
/// macOS entries are the same coverage for other platforms, so a missing file
/// is simply skipped.
const SYMBOL_FONTS: &[&str] = &[
    "C:\\Windows\\Fonts\\seguisym.ttf",
    "C:\\Windows\\Fonts\\cambria.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansMath-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
    "/System/Library/Fonts/Apple Symbols.ttf",
];

/// Fallback faces the bundled set lacks, in fallback order: symbols, emoji,
/// Chinese (simplified *and* traditional), Japanese, then Korean. Before this,
/// non-Latin text drew as empty boxes.
fn install_fonts(ctx: &egui::Context) {
    // The colour-emoji face is *not* in this chain: bubbles and the picker
    // rasterise emoji with `emoji_glyph` (swash), which reads the same file
    // directly. Keeping a second 12 MB copy here only cost memory.
    let groups: [(&str, &[&str]); 4] = [
        // Symbols and mathematical letters *before* the emoji face: a chat
        // titled "𝕋𝕒𝕦𝕗𝕚𝕢" (U+1D54B ...) used to draw as six missing-glyph
        // boxes because no face in the chain carried U+1D400-U+1D7FF. Segoe UI
        // Symbol ships with every Windows 10/11 install.
        ("system-symbols", SYMBOL_FONTS),
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
        diag(&format!("font {key}: {} kB", bytes.len() / 1024));
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
pub(crate) fn stamp_seconds(seconds: f32) -> String {
    let total = seconds.max(0.0) as i64;
    format!("{}:{:02}", total / 60, total % 60)
}

impl WaDesk {
    /// Channels: followed newsletters and their read-only posts.
    /// Channels: the official 64 px header, the "Discover channels" card and the
    /// followed list; opening a channel shows its posts read-only.
    fn channels_panel(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_min_height(40.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Channels")
                                    .size(20.0)
                                    .color(palette.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(ui, Icon::Close, &palette)
                                        .on_hover_text("Back to chats")
                                        .clicked()
                                    {
                                        self.show_channels = false;
                                    }
                                },
                            );
                        });
                    });

                // "Discover channels" - the official empty-state copy.
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Discover channels")
                                .size(17.0)
                                .color(palette.text),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "Entertainment, sports, news, lifestyle, people and more. \
                                 Follow the channels that interest you.",
                            )
                            .size(13.0)
                            .color(palette.secondary),
                        );
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if status_share_tile(ui, &palette, Icon::Add, "Create channel") {
                                self.notice = Some((
                                    "Creating channels needs the phone".to_string(),
                                    std::time::Instant::now(),
                                ));
                            }
                            if status_share_tile(ui, &palette, Icon::Search, "Explore channels") {
                                self.notice = Some((
                                    "Browsing the channel directory is not in this build"
                                        .to_string(),
                                    std::time::Instant::now(),
                                ));
                            }
                        });
                    });
                });

                let rows = self.model.channel_rows();
                if rows.is_empty() {
                    ui.add_space(26.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("No followed channels yet")
                                .size(13.5)
                                .color(palette.secondary),
                        );
                    });
                    return;
                }

                ui.add_space(16.0);
                ui.horizontal_top(|ui| {
                    // The channel rail.
                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        ui.set_width(240.0);
                        for (row, followers) in &rows {
                            let selected = self.model.active.as_deref() == Some(row.jid.as_str());
                            let card = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 64.0),
                                egui::Sense::click(),
                            );
                            let (rect, response) = card;
                            if selected || response.hovered() {
                                ui.painter().rect_filled(
                                    rect.shrink2(egui::vec2(8.0, 0.0)),
                                    egui::CornerRadius::same(12),
                                    if selected {
                                        palette.selected
                                    } else {
                                        palette.hover
                                    },
                                );
                            }
                            paint_avatar(ui, &row.name, 40.0, None);
                            ui.painter().text(
                                egui::pos2(rect.left() + 58.0, rect.center().y - 8.0),
                                egui::Align2::LEFT_CENTER,
                                fit_text(
                                    ui.painter(),
                                    &row.name,
                                    egui::FontId::proportional(14.5),
                                    palette.text,
                                    rect.width() - 70.0,
                                ),
                                egui::FontId::proportional(14.5),
                                palette.text,
                            );
                            ui.painter().text(
                                egui::pos2(rect.left() + 58.0, rect.center().y + 10.0),
                                egui::Align2::LEFT_CENTER,
                                followers.clone(),
                                egui::FontId::proportional(12.5),
                                palette.secondary,
                            );
                            if response.clicked() {
                                self.model.active = Some(row.jid.clone());
                            }
                            ui.add_space(2.0);
                        }
                    });
                    ui.separator();

                    // Posts, read-only: no composer for a channel.
                    ui.vertical(|ui| {
                        let Some(jid) = self.model.active.clone() else {
                            ui.add_space(20.0);
                            ui.label(
                                egui::RichText::new("Pick a channel")
                                    .size(13.0)
                                    .color(palette.secondary),
                            );
                            return;
                        };
                        let posts = self.model.channel_posts(&jid);
                        egui::ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for post in &posts {
                                    let header = format!(
                                        "{}  \u{00b7}  {}",
                                        self.model.title(
                                            &self
                                                .model
                                                .chats
                                                .get(&jid)
                                                .cloned()
                                                .unwrap_or_default()
                                        ),
                                        self.model.bubble_stamp(post)
                                    );
                                    ui.label(egui::RichText::new(header).small().weak());
                                    let body = self.model.bubble_body(post);
                                    egui::Frame::new()
                                        .fill(palette.bubble_in)
                                        .corner_radius(egui::CornerRadius::same(8))
                                        .inner_margin(egui::Margin::symmetric(10, 8))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(body)
                                                    .size(14.0)
                                                    .color(palette.text),
                                            );
                                        });
                                    ui.add_space(8.0);
                                }
                            });
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
                                        0 => self.profile_detail(ui, &palette),
                                        1 => self.account_detail(ui, &palette),
                                        2 => self.privacy_detail(ui, &palette),
                                        3 => self.chats_detail(ui, &palette),
                                        4 => self.notifications_detail(ui, &palette),
                                        5 => self.shortcuts_detail(ui, &palette),
                                        6 => self.ai_detail(ui, &palette),
                                        7 => self.updates_detail(ui, &palette),
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
            // The official list, in its order and with its subtitles (sampled
            // from the "You" drawer of web.whatsapp.com), plus this client's own
            // AI and Updates pages.
            let sections = [
                (Icon::Status, "Profile", "Name, profile picture, status"),
                (
                    Icon::Lock,
                    "Account",
                    "Security notifications, account info",
                ),
                (
                    Icon::SelectChats,
                    "Privacy",
                    "Blocked contacts, disappearing messages",
                ),
                (Icon::Chats, "Chats", "Theme, wallpaper, chat settings"),
                (Icon::Phone, "Notifications", "Messages, groups, sounds"),
                (
                    Icon::Edit,
                    "Keyboard shortcuts",
                    "Keys this window answers to",
                ),
                (Icon::Emoji, "AI assistant", "Auto-reply with your key"),
                (Icon::Document, "Updates", "Version and releases"),
                (Icon::Groups, "Help", "Project, changelog, privacy"),
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

    /// Profile: how the account appears to everyone else.
    fn profile_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Profile",
            "This is how you appear to your contacts.",
        );
        let me = self.model.me_jid.clone();
        self.ensure_avatar(ui.ctx(), &me);
        let avatar = self.avatar_for(&me).cloned();
        let name = self.model.own_name();
        let status = self.model.status.clone();
        settings_card(ui, palette, "PROFILE", |ui| {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                paint_avatar(ui, &name, 64.0, avatar.as_ref());
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&name).size(18.0).color(palette.text));
                    ui.label(
                        egui::RichText::new(&status)
                            .size(13.0)
                            .color(palette.secondary),
                    );
                });
            });
            ui.add_space(6.0);
        });
        settings_card(ui, palette, "NAME", |ui| {
            settings_row(
                ui,
                palette,
                "Your name",
                "Taken from the account's push name; change it on the phone.",
                |ui| {
                    ui.label(
                        egui::RichText::new(&name)
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Profile photo",
                "Fetched from WhatsApp and cached beside the session.",
                |ui| {
                    ui.label(
                        egui::RichText::new(if avatar.is_some() {
                            "Photo"
                        } else {
                            "Initials"
                        })
                        .size(13.0)
                        .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "ABOUT", |ui| {
            settings_row(
                ui,
                palette,
                "About",
                "Read-only: the phone owns this field.",
                |ui| {
                    ui.label(egui::RichText::new("—").size(13.0).color(palette.secondary));
                },
            );
        });
    }

    /// Account: the linked identity and what the session is doing.
    fn account_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Account",
            "Security notifications and this linked device.",
        );
        let me = self.model.me_jid.clone();
        let status = self.model.status.clone();
        let chats = self.model.chats.len();
        settings_card(ui, palette, "ACCOUNT INFO", |ui| {
            settings_row(
                ui,
                palette,
                "Phone number",
                "The JID this device is linked to.",
                |ui| {
                    let number = wa_ui::text::format_phone(&wa_ui::text::phone_from_jid(&me));
                    ui.label(
                        egui::RichText::new(number)
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Session",
                "Live state of the WhatsApp connection.",
                |ui| {
                    let color = if status.eq_ignore_ascii_case("connected") {
                        palette.accent
                    } else {
                        palette.danger
                    };
                    ui.label(egui::RichText::new(&status).size(13.0).color(color));
                },
            );
            settings_row(
                ui,
                palette,
                "Chats on this device",
                "Conversations stored in the local cache.",
                |ui| {
                    ui.label(
                        egui::RichText::new(chats.to_string())
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "LINKED DEVICES", |ui| {
            settings_row(
                ui,
                palette,
                "This device",
                "The session in this window; the phone owns the full device list.",
                |ui| {
                    ui.label(
                        egui::RichText::new("WA Desk")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Manage devices",
                "Remove a linked device from the phone: \
                 Settings -> Linked devices.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        // The live Account page (sampled 2026-11-05) lists exactly these three:
        // Security notifications, Request account info, How to delete my account.
        settings_card(ui, palette, "ACCOUNT", |ui| {
            settings_row(
                ui,
                palette,
                "Request account info",
                "The phone builds the report; this device can only ask.",
                |ui| {
                    if ui.button("Request").clicked() {
                        self.settings_note =
                            "Requesting account info is done from the phone".to_string();
                    }
                },
            );
            settings_row(
                ui,
                palette,
                "How to delete my account",
                "Deleting is a phone action; this link only explains it.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "SECURITY", |ui| {
            settings_row(
                ui,
                palette,
                "Security notifications",
                "A device is linked from the phone:                  Linked devices -> Link a device.",
                |ui| {
                    ui.label(
                        egui::RichText::new("On")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Two-step verification",
                "Managed by WhatsApp on the phone; not exposed to linked devices.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "STARTUP AND CLOSE", |ui| {
            let mut launch = crate::autostart::enabled();
            let mut changed = false;
            settings_row(
                ui,
                palette,
                "Launch WA Desk at login",
                "Starts with the account's Run key; nothing is written outside it.",
                |ui| {
                    if settings_switch(ui, palette, launch).clicked() {
                        launch = !launch;
                        changed = true;
                    }
                },
            );
            if changed {
                let note = match crate::autostart::set(launch) {
                    Ok(()) => format!(
                        "Launch at login {}.",
                        if launch { "enabled" } else { "disabled" }
                    ),
                    Err(err) => err,
                };
                self.settings_note = note;
            }
        });
        settings_card(ui, palette, "LOG OUT", |ui| {
            settings_row(
                ui,
                palette,
                "Unlink this device",
                "The phone keeps working; this session is removed.",
                |ui| {
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("Log out").color(palette.danger))
                                .frame(false),
                        )
                        .clicked()
                    {
                        self.send(Command::Relink);
                        self.settings_note =
                            "Unlinked - scan the QR with the phone to link again.".to_string();
                    }
                },
            );
        });
    }

    /// Privacy, in the official order: who can see what, disappearing messages,
    /// the block list and the advanced switches. Visibility is owned by the
    /// phone (a linked device can read it, not set it), so the values say so
    /// instead of inventing a local setting.
    fn privacy_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Privacy",
            "Who can see my personal info, disappearing messages and blocked contacts.",
        );
        // The rows the live client lists under "Who can see my personal info",
        // sampled 2026-11-05.
        settings_card(ui, palette, "WHO CAN SEE MY PERSONAL INFO", |ui| {
            for (title, note) in [
                (
                    "Last seen and online",
                    "Decided on the phone; this device only reads chats.",
                ),
                (
                    "Profile picture",
                    "The phone's setting applies to the linked device too.",
                ),
                ("About", "Owned by the phone."),
                ("Status", "Status privacy is set when a story is posted."),
            ] {
                settings_row(ui, palette, title, note, |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                });
            }
            settings_row(
                ui,
                palette,
                "Read receipts",
                "A linked device always acknowledges what it displays; group receipts are always sent.",
                |ui| {
                    ui.label(
                        egui::RichText::new("On")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "DISAPPEARING MESSAGES", |ui| {
            settings_row(
                ui,
                palette,
                "Default message timer",
                "Set per chat on the phone; the bubble list shows it.",
                |ui| {
                    ui.label(
                        egui::RichText::new("24 hours")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Groups",
                "Older bodies cannot be requested back once the timer passes.",
                |ui| {
                    ui.label(
                        egui::RichText::new("My contacts")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "CALLS AND CONTACTS", |ui| {
            settings_row(
                ui,
                palette,
                "Blocked contacts",
                "The phone owns the blocked list; this client does not mirror it.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "App lock",
                "Require a password to open WhatsApp on this device.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "ADVANCED", |ui| {
            for (title, note) in [
                (
                    "Block unknown account messages",
                    "The phone blocks messages from unknown accounts past a volume.",
                ),
                (
                    "Protect IP address in calls",
                    "Calls are relayed through WhatsApp when the phone enables it.",
                ),
                (
                    "Turn off link previews",
                    "Previews are generated by the sender's client, not by this device.",
                ),
            ] {
                settings_row(ui, palette, title, note, |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                });
            }
        });
    }

    /// Notifications: the toast switch and what it filters.
    fn notifications_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(ui, palette, "Notifications", "Messages, groups and sounds.");
        let mut notifications = self.settings.notifications;
        let mut changed = false;
        settings_card(ui, palette, "MESSAGES", |ui| {
            settings_row(
                ui,
                palette,
                "Desktop notifications",
                "Only for messages that arrive while the window is unfocused, and never                  for the open chat, statuses or channels - the official rule.",
                |ui| {
                    if settings_switch(ui, palette, notifications).clicked() {
                        notifications = !notifications;
                        changed = true;
                    }
                },
            );
            settings_row(
                ui,
                palette,
                "Test notification",
                "Posts one toast so the OS path can be checked.",
                |ui| {
                    if ui.button("Send").clicked() {
                        let _ = crate::notify::send("WA Desk", "Notifications are on.");
                    }
                },
            );
        });
        if changed {
            self.settings.notifications = notifications;
            self.settings_note = format!(
                "Desktop notifications {}.",
                if notifications { "enabled" } else { "disabled" }
            );
            let _ = self.settings.save(&self.settings_path);
        }
    }

    /// Keyboard shortcuts this window answers to.
    fn shortcuts_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Keyboard shortcuts",
            "Keys this window answers to.",
        );
        let rows = [
            ("Ctrl + V", "Paste an image or file into the composer"),
            ("Enter", "Send the message"),
            ("Shift + Enter", "New line in the composer"),
            ("Esc", "Close search, the viewer or the emoji picker"),
            ("Arrow keys", "Previous / next media in the viewer"),
            ("Space", "Play or pause the open clip"),
            ("Ctrl + F", "Search inside the open chat"),
        ];
        settings_card(ui, palette, "SHORTCUTS", |ui| {
            for (keys, what) in rows {
                settings_row(ui, palette, keys, what, |ui| {
                    ui.label("");
                });
            }
        });
    }

    /// The About / Storage card at the bottom of the Help page.
    fn about_card(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
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
        settings_card(ui, palette, "CHATS", |ui| {
            // The live Chats pane (sampled 2026-11-05) lists these five under
            // the theme choices; ours reports what this client actually does.
            for (title, note, value) in [
                (
                    "Media upload quality",
                    "Photos and videos are uploaded untouched.",
                    "Original",
                ),
                (
                    "Media auto-download",
                    "A chat's media is fetched when it is opened.",
                    "On open",
                ),
                ("Spell check", "egui has no spell checker.", "Off"),
                (
                    "Replace text with emoji",
                    "Typed shortcuts are sent as typed.",
                    "Off",
                ),
                ("Enter is send", "Enter sends the message.", "On"),
            ] {
                settings_row(ui, palette, title, note, |ui| {
                    ui.label(
                        egui::RichText::new(value)
                            .size(13.0)
                            .color(palette.secondary),
                    );
                });
            }
        });
        ui.add_space(12.0);
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

        ui.add_space(12.0);
        self.about_card(ui, palette);
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

/// The "Unread messages" divider the official client puts above the first
/// unread bubble: a hairline with a centered pill.
fn unread_divider(ui: &mut egui::Ui, palette: &crate::theme::Palette) {
    let galley = ui.painter().layout_no_wrap(
        "Unread messages".to_string(),
        egui::FontId::proportional(11.5),
        palette.secondary,
    );
    let width = galley.size().x + 22.0;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let height = 18.0;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            egui::Sense::hover(),
        );
        let pill = egui::Rect::from_center_size(rect.center(), egui::vec2(width, height));
        let stroke = egui::Stroke::new(1.0_f32, palette.border);
        ui.painter().line_segment(
            [rect.left_center(), egui::pos2(pill.left(), rect.center().y)],
            stroke,
        );
        ui.painter().line_segment(
            [
                egui::pos2(pill.right(), rect.center().y),
                rect.right_center(),
            ],
            stroke,
        );
        ui.painter()
            .rect_filled(pill, egui::CornerRadius::same(9), palette.elevated);
        ui.painter().galley(
            egui::pos2(
                pill.center().x - galley.size().x / 2.0,
                pill.center().y - galley.size().y / 2.0,
            ),
            galley.clone(),
            palette.secondary,
        );
    });
    ui.add_space(4.0);
}

/// The poll composer's state.
#[derive(Clone, Default)]
pub struct PollDraft {
    pub question: String,
    pub options: [String; 3],
    pub multiple: bool,
}

/// The location composer's state.
#[derive(Clone, Default)]
pub struct LocationDraft {
    pub name: String,
    pub lat: String,
    pub lng: String,
}

/// The contact-card composer's state.
#[derive(Clone, Default)]
pub struct ContactDraft {
    pub name: String,
    pub phone: String,
}

/// One 72 px call row: avatar, name, direction + time, and a call button.
/// Returns `Some(video)` when the call button was pressed (false = voice).
fn call_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    entry: &crate::model::CallLogEntry,
    avatar: Option<&egui::TextureHandle>,
) -> Option<bool> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 72.0), egui::Sense::click());
    let card = rect.shrink2(egui::vec2(8.0, 0.0));
    if response.hovered() {
        ui.painter()
            .rect_filled(card, egui::CornerRadius::same(12), palette.hover);
    }
    let center = egui::pos2(rect.left() + 44.0, rect.center().y);
    if let Some(texture) = avatar {
        ui.painter().image(
            texture.id(),
            egui::Rect::from_center_size(center, egui::vec2(48.0, 48.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        ui.painter()
            .circle_filled(center, 24.0, avatar_color(&entry.name));
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            initials(&entry.name),
            egui::FontId::proportional(16.0),
            egui::Color32::WHITE,
        );
    }
    // Missed calls are red in the official list; the rest are secondary.
    let name_color = if entry.missed {
        palette.danger
    } else {
        palette.text
    };
    ui.painter().text(
        egui::pos2(rect.left() + 82.0, rect.top() + 26.0),
        egui::Align2::LEFT_CENTER,
        fit_text(
            ui.painter(),
            &entry.name,
            egui::FontId::proportional(15.0),
            name_color,
            rect.width() - 200.0,
        ),
        egui::FontId::proportional(15.0),
        name_color,
    );
    // Direction glyph + time on the second line.
    let glyph = if entry.missed {
        Icon::CallEnd
    } else if entry.outgoing {
        Icon::Forward
    } else {
        Icon::Reply
    };
    let glyph_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 82.0, rect.top() + 44.0),
        egui::vec2(14.0, 14.0),
    );
    draw_icon(ui, glyph_rect, glyph, name_color);
    let stamp = if entry.at > 0 {
        wa_ui::labels::stamp_of(entry.at, now_ms())
    } else {
        String::new()
    };
    let kind = if entry.video {
        "Video call"
    } else {
        "Voice call"
    };
    let detail = if entry.missed {
        stamp
    } else if entry.seconds > 0 {
        let minutes = entry.seconds / 60;
        let seconds = entry.seconds % 60;
        format!("{stamp}  ·  {kind}  ·  {minutes}:{seconds:02}")
    } else {
        format!("{stamp}  ·  {kind}")
    };
    ui.painter().text(
        egui::pos2(rect.left() + 101.0, rect.top() + 51.0),
        egui::Align2::LEFT_CENTER,
        detail,
        egui::FontId::proportional(12.5),
        palette.secondary,
    );

    // The call button on the right.
    let button = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 34.0, rect.center().y),
        egui::vec2(34.0, 34.0),
    );
    let hit = ui.interact(
        button,
        ui.id().with(("call", &entry.jid)),
        egui::Sense::click(),
    );
    if hit.hovered() {
        ui.painter()
            .rect_filled(button, egui::CornerRadius::same(17), palette.hover);
    }
    draw_icon(
        ui,
        button.shrink(8.0),
        if entry.video {
            Icon::VideoCall
        } else {
            Icon::Phone
        },
        palette.accent,
    );
    if hit.clicked() {
        return Some(entry.video);
    }
    None
}

/// One 72 px status row: ring avatar, name, meta line.
fn status_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    name: &str,
    meta: &str,
    avatar: Option<&egui::TextureHandle>,
    unseen: bool,
    mine: bool,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 72.0), egui::Sense::click());
    let card = rect.shrink2(egui::vec2(8.0, 0.0));
    if response.hovered() {
        ui.painter()
            .rect_filled(card, egui::CornerRadius::same(12), palette.hover);
    }
    let center = egui::pos2(rect.left() + 44.0, rect.center().y);
    let ring = if mine {
        palette.secondary
    } else if unseen {
        palette.accent
    } else {
        egui::Color32::from_gray(120)
    };
    ui.painter()
        .circle_stroke(center, 26.0, egui::Stroke::new(2.5_f32, ring));
    if let Some(texture) = avatar {
        ui.painter().image(
            texture.id(),
            egui::Rect::from_center_size(center, egui::vec2(46.0, 46.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        ui.painter().circle_filled(center, 23.0, avatar_color(name));
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            initials(name),
            egui::FontId::proportional(16.0),
            egui::Color32::WHITE,
        );
    }
    ui.painter().text(
        egui::pos2(rect.left() + 82.0, rect.top() + 24.0),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(15.0),
        palette.text,
    );
    let meta_text = if mine && meta.is_empty() {
        "Click to add status update"
    } else {
        meta
    };
    ui.painter().text(
        egui::pos2(rect.left() + 82.0, rect.top() + 46.0),
        egui::Align2::LEFT_CENTER,
        meta_text,
        egui::FontId::proportional(13.0),
        palette.secondary,
    );
    response.clicked()
}

/// The chat wallpaper: WhatsApp's faint doodle sheet, drawn once into a tile
/// and repeated behind the messages. Official dark is `#0B141A` with `#182229`
/// doodles; the light theme uses the paper sheet with `#EFE7DD` doodles.
fn wallpaper_textile(ctx: &egui::Context, palette: &crate::theme::Palette) -> egui::TextureHandle {
    let key = format!(
        "wallpaper:{:02x}{:02x}{:02x}",
        palette.wallpaper.r(),
        palette.wallpaper.g(),
        palette.wallpaper.b()
    );
    if let Some(texture) =
        ctx.data(|data| data.get_temp::<egui::TextureHandle>(egui::Id::new(&key)))
    {
        return texture;
    }
    const SIDE: usize = 220;
    let base = palette.wallpaper;
    let ink = if palette.dark {
        egui::Color32::from_rgb(17, 25, 31)
    } else {
        egui::Color32::from_rgb(240, 233, 224)
    };
    let mut pixels = vec![base; SIDE * SIDE];
    let plot = |pixels: &mut Vec<egui::Color32>, x: i32, y: i32, radius: i32| {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let px = x + dx;
                let py = y + dy;
                if px < 0 || py < 0 || px >= SIDE as i32 || py >= SIDE as i32 {
                    continue;
                }
                pixels[py as usize * SIDE + px as usize] = ink;
            }
        }
    };
    // A deterministic scatter of small doodles: rings, blocks and dashes.
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    for _ in 0..26 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let x = (seed % SIDE as u64) as i32;
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let y = (seed % SIDE as u64) as i32;
        match seed % 3 {
            0 => {
                for angle in 0..40 {
                    let step = angle as f32 * std::f32::consts::TAU / 40.0;
                    plot(
                        &mut pixels,
                        x + (step.cos() * 7.0) as i32,
                        y + (step.sin() * 7.0) as i32,
                        1,
                    );
                }
            }
            1 => {
                for dy in 0..4 {
                    for dx in 0..7 {
                        plot(&mut pixels, x + dx, y + dy, 0);
                    }
                }
            }
            _ => {
                for dx in 0..11 {
                    plot(&mut pixels, x + dx, y, 0);
                }
            }
        }
    }
    let image = egui::ColorImage {
        size: [SIDE, SIDE],
        source_size: egui::vec2(SIDE as f32, SIDE as f32),
        pixels,
    };
    let texture = ctx.load_texture(key.clone(), image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(&key), texture.clone()));
    texture
}

/// Paint the wallpaper tile across `area`, like a tiled background image.
fn paint_wallpaper(ui: &egui::Ui, palette: &crate::theme::Palette, area: egui::Rect) {
    let texture = wallpaper_textile(ui.ctx(), palette);
    let side = 220.0;
    let painter = ui.painter().with_clip_rect(area);
    let mut y = area.top();
    while y < area.bottom() {
        let mut x = area.left();
        while x < area.right() {
            painter.image(
                texture.id(),
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(side, side)),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            x += side;
        }
        y += side;
    }
}

/// The initials for an avatar fallback ("Ada Lovelace" -> "AL").
pub(crate) fn call_initials(name: &str) -> String {
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

impl WaDesk {
    /// Group info: subject, description, member count and the member list
    /// with admin chips.
    /// Group info, in the official shape: a 64 px header, the group's avatar and
    /// name, the action cards, the member list with admin labels and Exit group.
    fn group_panel(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_min_height(40.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Group info")
                                    .size(20.0)
                                    .color(palette.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Back").clicked() {
                                        self.show_group = false;
                                    }
                                },
                            );
                        });
                    });

                if let Some(err) = self.model.group_error.clone() {
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label(egui::RichText::new(err).color(palette.danger));
                    });
                    return;
                }
                let Some(info) = self.model.group.clone() else {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("Loading group info…")
                                .size(13.5)
                                .color(palette.secondary),
                        );
                    });
                    return;
                };

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Avatar, name, kind.
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            let avatar = self.avatar_for(&info.jid).cloned();
                            paint_avatar(ui, &info.subject, 96.0, avatar.as_ref());
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new(&info.subject)
                                    .size(21.0)
                                    .color(palette.text),
                            );
                            // `group_member_count` already reads
                            // "Group · N members"; a community only needs its own
                            // name in front.
                            let count = wa_ui::labels::group_member_count(info.members.len());
                            let label = if info.is_community {
                                format!("Community · {count}")
                            } else {
                                count
                            };
                            ui.label(
                                egui::RichText::new(label)
                                    .size(13.0)
                                    .color(palette.secondary),
                            );
                            ui.add_space(14.0);
                        });

                        let width = (ui.available_width() - 40.0).clamp(280.0, 640.0);
                        let side = ((ui.available_width() - width) / 2.0).max(0.0);
                        ui.horizontal(|ui| {
                            ui.add_space(side);
                            ui.vertical(|ui| {
                                ui.set_width(width);
                                if let Some(description) =
                                    info.description.clone().filter(|d| !d.trim().is_empty())
                                {
                                    settings_card(ui, &palette, "DESCRIPTION", |ui| {
                                        ui.add_space(6.0);
                                        ui.label(
                                            egui::RichText::new(description)
                                                .size(13.5)
                                                .color(palette.text),
                                        );
                                        ui.add_space(6.0);
                                    });
                                    ui.add_space(10.0);
                                }
                                settings_card(ui, &palette, "GROUP", |ui| {
                                    settings_row(
                                        ui,
                                        &palette,
                                        "Add members",
                                        "Pick contacts from the address book.",
                                        |_ui| {},
                                    );
                                    settings_row(
                                        ui,
                                        &palette,
                                        "Media, links and docs",
                                        "Everything shared in this group.",
                                        |_ui| {},
                                    );
                                    settings_row(
                                        ui,
                                        &palette,
                                        "Group settings",
                                        "Send messages, edit info, disappearing messages.",
                                        |_ui| {},
                                    );
                                });
                                ui.add_space(10.0);
                                settings_card(
                                    ui,
                                    &palette,
                                    &format!("{} MEMBERS", info.members.len()),
                                    |ui| {
                                        for member in &info.members {
                                            let name = member
                                                .phone
                                                .clone()
                                                .map(|phone| wa_ui::text::format_phone(&phone))
                                                .unwrap_or_else(|| {
                                                    member
                                                        .jid
                                                        .split('@')
                                                        .next()
                                                        .unwrap_or(&member.jid)
                                                        .to_string()
                                                });
                                            self.ensure_avatar(ui.ctx(), &member.jid);
                                            let avatar = self.avatar_for(&member.jid).cloned();
                                            ui.horizontal(|ui| {
                                                ui.add_space(10.0);
                                                paint_avatar(ui, &name, 40.0, avatar.as_ref());
                                                ui.add_space(6.0);
                                                ui.vertical(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(&name)
                                                            .size(14.5)
                                                            .color(palette.text),
                                                    );
                                                    if member.admin {
                                                        ui.label(
                                                            egui::RichText::new("Group admin")
                                                                .size(12.0)
                                                                .color(palette.secondary),
                                                        );
                                                    }
                                                });
                                            });
                                            ui.add_space(8.0);
                                        }
                                    },
                                );
                                ui.add_space(10.0);
                                settings_card(ui, &palette, "LEAVE", |ui| {
                                    settings_row(
                                        ui,
                                        &palette,
                                        "Exit group",
                                        "You will stop receiving messages from this group.",
                                        |ui| {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("Exit")
                                                            .color(palette.danger),
                                                    )
                                                    .frame(false),
                                                )
                                                .clicked()
                                            {
                                                self.send(Command::LeaveGroup {
                                                    chat: info.jid.clone(),
                                                });
                                                self.show_group = false;
                                            }
                                        },
                                    );
                                });
                                ui.add_space(24.0);
                            });
                        });
                    });
            });
    }
}

impl eframe::App for WaDesk {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.smoke_send();
        // Texture caches are capped so memory does not grow with a long session.
        prune_textures(ctx, &mut self.textures, 96);
        prune_textures(ctx, &mut self.sticker_textures, 240);
        if let Some(recording) = self.voice.as_mut() {
            recording.pump();
        }
        // Frame pacing: while a clip plays, average the gap between paints and
        // report it once per 30 frames, so "smooth" is a measured number.
        let paint_now = std::time::Instant::now();
        let gap_ms = paint_now.duration_since(self.last_paint).as_secs_f32() * 1000.0;
        self.last_paint = paint_now;
        if self.video_playing && self.model.media_viewer.is_some() {
            // A resume frame carries the whole pause; ignore anything long.
            if gap_ms < 250.0 {
                self.paint_gap_sum += gap_ms;
                self.paint_gap_count += 1;
            }
            if self.paint_gap_count >= 30 {
                let average = self.paint_gap_sum / self.paint_gap_count as f32;
                diag(&format!(
                    "paint: {:.1} ms average over {} frames ({:.1} fps)",
                    average,
                    self.paint_gap_count,
                    1000.0 / average.max(0.01),
                ));
                self.paint_gap_sum = 0.0;
                self.paint_gap_count = 0;
            }
        } else {
            self.paint_gap_sum = 0.0;
            self.paint_gap_count = 0;
        }
        // `WA_DESK_DIAG=1` gets a line every ten seconds, so a long session's
        // growth (or lack of it) can be read from diagnostics.log.
        if self.last_memory_log.elapsed() >= std::time::Duration::from_secs(10) {
            self.last_memory_log = std::time::Instant::now();
            diag(&format!(
                "memory: textures {} stickers {} video-frames {} chats {} messages {}",
                self.textures.len(),
                self.sticker_textures.len(),
                self.video_textures.len(),
                self.model.chats.len(),
                self.model
                    .chats
                    .values()
                    .map(|row| row.messages.len())
                    .sum::<usize>(),
            ));
        }
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
        // `WA_DESK_OPEN_CHAT=<text>` opens the first chat whose title contains it
        // (diagnosing a specific conversation without clicking).
        if let Ok(needle) = std::env::var("WA_DESK_OPEN_CHAT") {
            let needle = needle.to_lowercase();
            if let Some(jid) = self
                .model
                .visible_chats()
                .iter()
                .find(|row| self.model.title(row).to_lowercase().contains(&needle))
                .map(|row| row.jid.clone())
            {
                self.model.open_chat(&jid);
                self.send(Command::MarkRead { chat: jid.clone() });
                self.send(Command::ChatHistory { chat: jid });
                self.auto_opened = true;
                // Opened once; auto_opened keeps it from repeating.
            }
        }
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
            } else if self.show_communities {
                self.communities_panel(ctx);
            } else if self.show_calls {
                self.calls_panel(ctx);
            } else if self.show_status {
                self.status_panel(ctx);
            } else {
                self.conversation(ctx);
                self.emoji_window(ctx);
                self.attach_window(ctx);
                self.poll_window(ctx);
                self.contact_window(ctx);
                self.location_window(ctx);
                self.list_window(ctx);
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
            let media_cache = self.cache_path.with_file_name("media.json");
            let _ = std::fs::write(&media_cache, self.model.media_cache_json());
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
        let gif_on_screen = crate::gif::any_prepared()
            && self
                .model
                .active_row()
                .is_some_and(|row| row.messages.iter().any(|message| message.gif));
        let sticker_on_screen = crate::webp_anim::any_ready()
            && self
                .model
                .active_row()
                .is_some_and(|row| row.messages.iter().any(|message| message.kind == "sticker"));
        if self.audio.state().playing || self.video_playing || gif_on_screen || sticker_on_screen {
            // An animated sticker knows its own frame duration; otherwise the
            // 100 ms tick is enough for GIFs and audio meters.
            let per_frame = if sticker_on_screen {
                crate::webp_anim::shortest_frame_ms()
                    .map(|ms| ms.clamp(16, 100))
                    .unwrap_or(100)
            } else {
                100
            };
            ctx.request_repaint_after(std::time::Duration::from_millis(u64::from(per_frame)));
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
        let mut gif_send: Option<String> = None;
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
                                    // GIFs of this chat, newest first: WhatsApp's
                                    // picker sends a recent one again.
                                    let gifs = self.model.gif_items();
                                    if gifs.is_empty() {
                                        ui.add_space(10.0);
                                        ui.label(
                                            egui::RichText::new(
                                                "No GIFs in this chat yet - a GIF someone \
                                                 sends shows up here.",
                                            )
                                            .size(12.5)
                                            .color(palette.secondary),
                                        );
                                    } else {
                                        ui.label(
                                            egui::RichText::new("GIFs in this chat")
                                                .size(11.5)
                                                .strong()
                                                .color(palette.secondary),
                                        );
                                        ui.add_space(4.0);
                                        ui.horizontal_wrapped(|ui| {
                                            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                            for (id, path) in &gifs {
                                                let (rect, response) = ui.allocate_exact_size(
                                                    egui::vec2(108.0, 108.0),
                                                    egui::Sense::click(),
                                                );
                                                if response.hovered() {
                                                    ui.painter().rect_filled(
                                                        rect,
                                                        egui::CornerRadius::same(10),
                                                        palette.hover,
                                                    );
                                                }
                                                let key = format!("gif:{id}");
                                                if let Some(texture) = texture_at(
                                                    ui.ctx(),
                                                    &mut self.textures,
                                                    &key,
                                                    (!path.is_empty()).then(|| path.clone()),
                                                    QUOTE_PX,
                                                ) {
                                                    let size = texture.size_vec2();
                                                    let scale = (100.0_f32 / size.x.max(1.0))
                                                        .min(100.0 / size.y.max(1.0));
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
                                                } else {
                                                    ui.painter().text(
                                                        rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        "GIF",
                                                        egui::FontId::proportional(14.0),
                                                        palette.secondary,
                                                    );
                                                }
                                                if response.clicked() {
                                                    gif_send = Some(id.clone());
                                                }
                                            }
                                        });
                                    }
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
        if let Some(id) = gif_send
            && let Some(chat) = self.model.active.clone()
        {
            self.send(Command::ForwardMessage { chat, id });
            self.model.emoji_open = false;
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
        // 40 px cell with a 30 px glyph: the official grid breathes.
        let (rect, response) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(8), palette.hover);
        }
        let glyph = emoji.chars().next().unwrap_or('?');
        if let Some(texture) = self.emoji_texture(ui.ctx(), glyph, 30) {
            let size = texture.size_vec2();
            let scale = (30.0 / size.x.max(1.0)).min(30.0 / size.y.max(1.0));
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
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
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
        // The sticker the user just tapped, so its bubble can appear before
        // the upload finishes.
        let mut sticker_preview: Option<(String, String)> = None;
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
                                if self.model.packs.is_empty() && !self.packs_requested {
                                    self.packs_requested = true;
                                    self.send(Command::StickerPacks);
                                }
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
                                    // Packs shared in a chat: WhatsApp never syncs the
                                    // account's pack list to linked devices.
                                    if self.model.packs.is_empty() {
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new(
                                                "No sticker packs yet - a pack shows up \
                                                 here when someone shares one in a chat.",
                                            )
                                            .size(12.5)
                                            .color(palette.secondary),
                                        );
                                        ui.add_space(6.0);
                                        if settings_link(ui, &palette, "Check again")
                                            && !self.packs_requested
                                        {
                                            self.packs_requested = true;
                                            self.send(Command::StickerPacks);
                                        }
                                    } else {
                                        let packs = self.model.packs.clone();
                                        let active = self.model.active_pack.clone();
                                        ui.add_space(6.0);
                                        for pack in &packs {
                                            let selected =
                                                active.as_deref() == Some(pack.id.as_str());
                                            let label = format!(
                                                "{} - {} stickers",
                                                if pack.publisher.is_empty() {
                                                    pack.name.clone()
                                                } else {
                                                    format!("{} ({})", pack.name, pack.publisher)
                                                },
                                                pack.stickers.len()
                                            );
                                            let (rect, response) = ui.allocate_exact_size(
                                                egui::vec2(ui.available_width(), 30.0),
                                                egui::Sense::click(),
                                            );
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
                                                egui::pos2(rect.left() + 8.0, rect.center().y),
                                                egui::Align2::LEFT_CENTER,
                                                &label,
                                                egui::FontId::proportional(12.5),
                                                palette.text,
                                            );
                                            if response.clicked() {
                                                self.model.active_pack = Some(pack.id.clone());
                                                self.send(Command::StickerPackItems {
                                                    id: pack.id.clone(),
                                                });
                                            }
                                        }
                                        ui.add_space(6.0);
                                        let stickers = self.model.active_pack_stickers();
                                        if stickers.iter().all(|item| item.path.is_empty()) {
                                            ui.label(
                                                egui::RichText::new("Downloading the pack...")
                                                    .size(12.0)
                                                    .color(palette.secondary),
                                            );
                                        } else {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.spacing_mut().item_spacing =
                                                    egui::vec2(6.0, 6.0);
                                                for item in &stickers {
                                                    if item.path.is_empty() {
                                                        continue;
                                                    }
                                                    let key = format!("pack:{}", item.path);
                                                    let Some(texture) = texture_at(
                                                        ui.ctx(),
                                                        &mut self.sticker_textures,
                                                        &key,
                                                        Some(item.path.clone()),
                                                        STICKER_PX,
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
                                                    let scale = (66.0_f32 / size.x.max(1.0))
                                                        .min(66.0 / size.y.max(1.0));
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
                                                        use base64::Engine as _;
                                                        sticker_preview =
                                                            Some((chat.clone(), item.path.clone()));
                                                        let bytes = std::fs::read(&item.path)
                                                            .unwrap_or_default();
                                                        send = Some(Command::SendSticker {
                                                            chat: chat.clone(),
                                                            payload:
                                                                base64::engine::general_purpose::STANDARD
                                                                    .encode(bytes),
                                                        });
                                                    }
                                                }
                                            });
                                        }
                                    }
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
                                            // The official Create tile turns a picture
                                            // into a 512x512 WebP and sends it.
                                            if let Some(path) = rfd::FileDialog::new()
                                                .add_filter(
                                                    "Image",
                                                    &["png", "jpg", "jpeg", "webp"],
                                                )
                                                .pick_file()
                                            {
                                                match crate::sticker::make_sticker(&path) {
                                                    Ok(bytes) => {
                                                        use base64::Engine as _;
                                                        if !chat.is_empty() {
                                                            send = Some(Command::SendSticker {
                                                                chat: chat.clone(),
                                                                payload:
                                                                    base64::engine::general_purpose::STANDARD
                                                                        .encode(bytes),
                                                            });
                                                        }
                                                    }
                                                    Err(err) => {
                                                        self.notice = Some((
                                                            err,
                                                            std::time::Instant::now(),
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    for (id, path, favorite) in &rows {
                                        let key = format!("sticker:{id}");
                                        let Some(texture) = texture_at(
                                            ui.ctx(),
                                            &mut self.sticker_textures,
                                            &key,
                                            Some(path.clone()),
                                            STICKER_PX,
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
                                            (66.0_f32 / size.x.max(1.0)).min(66.0 / size.y.max(1.0));
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
                                            sticker_preview =
                                                Some((chat.clone(), path.clone()));
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
                            ui.add_space(10.0);
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
            if let Some((chat, path)) = sticker_preview {
                self.model.add_own_sticker(&chat, &path);
            }
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

    /// `WA_DESK_SEND_ONCE="<chat title>|<text>"` sends one message once the chat
    /// list is loaded. It exists so the send path can be smoke-tested against a
    /// real session without driving the mouse; it never runs unless it is set.
    fn smoke_send(&mut self) {
        if self.smoke_sent {
            return;
        }
        let Ok(spec) = std::env::var("WA_DESK_SEND_ONCE") else {
            return;
        };
        let Some((title, body)) = spec.split_once('|') else {
            return;
        };
        let body = body.trim();
        if body.is_empty() {
            return;
        }
        let Some(jid) = self
            .model
            .chats
            .values()
            .find(|row| self.model.row_title(row) == title.trim())
            .map(|row| row.jid.clone())
        else {
            return;
        };
        self.smoke_sent = true;
        self.model.open_chat(&jid);
        self.model.add_own_text(&jid, body, None);
        self.send(Command::SendText {
            chat: jid,
            text: body.to_string(),
        });
        diag(&format!("smoke send to {title}: {body}"));
    }

    /// The forward picker, in the official shape: a floating card with the close
    /// control and the title on top, the send action pinned right, a rounded
    /// search field, the "My status" row, and the recent chats with a square
    /// checkbox each.
    fn forward_window(&mut self, ctx: &egui::Context) {
        let Some(target) = self.model.forward_target.clone() else {
            return;
        };
        // Snapshot the mutable bits: the card paints and collects actions, the
        // model is only touched once the card is closed.
        let palette = self.palette.clone();
        let mut search = self.model.forward_search.clone();
        let mut chosen = self.model.forward_chosen.clone();
        let mut toggle: Option<String> = None;
        let mut send = false;
        let mut close = false;
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.model.forward_target = None;
            return;
        }

        let viewport = ctx.screen_rect();
        let width = 400.0_f32.min(viewport.width() - 40.0);
        let height = (viewport.height() * 0.74).min(640.0);

        // Avatars are fetched while the rows are collected, so the card itself
        // only paints (a `&mut self` inside the area closure would fight the
        // model borrows).
        let jids: Vec<String> = self
            .model
            .forward_rows()
            .iter()
            .map(|row| row.jid.clone())
            .collect();
        let mut rows: Vec<(String, String, String, Option<egui::TextureHandle>)> = Vec::new();
        for jid in jids {
            self.ensure_avatar(ctx, &jid);
            let (title, preview) = match self.model.chats.get(&jid) {
                Some(row) => (self.model.row_title(row), self.model.row_preview(row)),
                None => (jid.clone(), String::new()),
            };
            let avatar = self.avatar_for(&jid).cloned();
            rows.push((jid, title, preview, avatar));
        }

        egui::Area::new(egui::Id::new("forward-card"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(
                viewport.center().x - width / 2.0,
                viewport.center().y - height / 2.0,
            ))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .stroke(egui::Stroke::new(1.0_f32, palette.border))
                    .corner_radius(egui::CornerRadius::same(16))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 12],
                        blur: 32,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(150),
                    })
                    .inner_margin(egui::Margin::symmetric(18, 14))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.set_height(height);

                        // Header: close, title, send.
                        ui.horizontal(|ui| {
                            let (rect, response) = ui
                                .allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::click());
                            if response.hovered() {
                                ui.painter()
                                    .circle_filled(rect.center(), 16.0, palette.hover);
                            }
                            draw_icon(
                                ui,
                                egui::Rect::from_center_size(rect.center(), egui::vec2(18.0, 18.0)),
                                Icon::Close,
                                palette.text,
                            );
                            if response
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                close = true;
                            }
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("Forward message to")
                                    .size(16.0)
                                    .color(palette.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let ready = !chosen.is_empty();
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(32.0, 32.0),
                                        egui::Sense::click(),
                                    );
                                    if ready && response.hovered() {
                                        ui.painter().circle_filled(
                                            rect.center(),
                                            16.0,
                                            palette.hover,
                                        );
                                    }
                                    draw_icon(
                                        ui,
                                        egui::Rect::from_center_size(
                                            rect.center(),
                                            egui::vec2(20.0, 20.0),
                                        ),
                                        Icon::Send,
                                        if ready {
                                            palette.accent
                                        } else {
                                            palette.secondary
                                        },
                                    );
                                    if ready
                                        && response
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .clicked()
                                    {
                                        send = true;
                                    }
                                },
                            );
                        });
                        ui.add_space(12.0);

                        // The search field: rounded, accent hairline, magnifier
                        // inside like the official box.
                        egui::Frame::new()
                            .fill(palette.panel)
                            .stroke(egui::Stroke::new(1.0_f32, palette.accent))
                            .corner_radius(egui::CornerRadius::same(12))
                            .inner_margin(egui::Margin::symmetric(14, 7))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(18.0, 18.0),
                                        egui::Sense::hover(),
                                    );
                                    draw_icon(ui, rect, Icon::Search, palette.accent);
                                    ui.add_space(4.0);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut search)
                                            .frame(false)
                                            .hint_text("Search name, number or @username")
                                            .desired_width(ui.available_width()),
                                    );
                                });
                            });
                        ui.add_space(10.0);

                        // "My status": the official second row. This client
                        // posts statuses from the Status screen, so the row is
                        // shown but not selectable here.
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 58.0),
                            egui::Sense::hover(),
                        );
                        if response.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                egui::CornerRadius::same(10),
                                palette.hover,
                            );
                        }
                        forward_checkbox(ui, &palette, rect.left() + 12.0, rect.center().y, false);
                        let disc = egui::pos2(rect.left() + 52.0, rect.center().y);
                        ui.painter().circle_filled(disc, 20.0, palette.accent);
                        draw_icon(
                            ui,
                            egui::Rect::from_center_size(disc, egui::vec2(20.0, 20.0)),
                            Icon::Status,
                            egui::Color32::from_rgb(11, 20, 26),
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + 82.0, rect.center().y - 9.0),
                            egui::Align2::LEFT_CENTER,
                            "My status",
                            egui::FontId::proportional(14.5),
                            palette.text,
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + 82.0, rect.center().y + 10.0),
                            egui::Align2::LEFT_CENTER,
                            "Contacts excluded",
                            egui::FontId::proportional(12.5),
                            palette.secondary,
                        );

                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("Recent chats")
                                .size(13.0)
                                .color(palette.secondary),
                        );
                        ui.add_space(4.0);

                        let list_height = (ui.available_height() - 34.0).max(120.0);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_height(list_height)
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                            )
                            .show(ui, |ui| {
                                for (jid, name, preview, avatar) in &rows {
                                    let picked = chosen.contains(jid);
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 60.0),
                                        egui::Sense::click(),
                                    );
                                    if response.hovered() {
                                        ui.painter().rect_filled(
                                            rect,
                                            egui::CornerRadius::same(10),
                                            palette.hover,
                                        );
                                    }
                                    forward_checkbox(
                                        ui,
                                        &palette,
                                        rect.left() + 12.0,
                                        rect.center().y,
                                        picked,
                                    );
                                    let disc = egui::pos2(rect.left() + 54.0, rect.center().y);
                                    if let Some(texture) = avatar {
                                        ui.painter().image(
                                            texture.id(),
                                            egui::Rect::from_center_size(
                                                disc,
                                                egui::vec2(40.0, 40.0),
                                            ),
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            egui::Color32::WHITE,
                                        );
                                    } else {
                                        let clean = name.trim_start_matches('~').trim();
                                        ui.painter().circle_filled(
                                            disc,
                                            20.0,
                                            crate::widgets::avatar_color(clean),
                                        );
                                        ui.painter().text(
                                            disc,
                                            egui::Align2::CENTER_CENTER,
                                            crate::widgets::initials(clean),
                                            egui::FontId::proportional(14.0),
                                            egui::Color32::WHITE,
                                        );
                                    }
                                    ui.painter().text(
                                        egui::pos2(rect.left() + 84.0, rect.center().y - 9.0),
                                        egui::Align2::LEFT_CENTER,
                                        fit_text(
                                            ui.painter(),
                                            name,
                                            egui::FontId::proportional(14.5),
                                            palette.text,
                                            rect.width() - 100.0,
                                        ),
                                        egui::FontId::proportional(14.5),
                                        palette.text,
                                    );
                                    ui.painter().text(
                                        egui::pos2(rect.left() + 84.0, rect.center().y + 10.0),
                                        egui::Align2::LEFT_CENTER,
                                        fit_text(
                                            ui.painter(),
                                            &crate::widgets::single_line(preview),
                                            egui::FontId::proportional(12.5),
                                            palette.secondary,
                                            rect.width() - 100.0,
                                        ),
                                        egui::FontId::proportional(12.5),
                                        palette.secondary,
                                    );
                                    if response
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        toggle = Some(jid.clone());
                                    }
                                }
                            });

                        if let Some(jid) = &toggle {
                            if let Some(index) = chosen.iter().position(|item| item == jid) {
                                chosen.remove(index);
                            } else {
                                chosen.push(jid.clone());
                            }
                        }
                    });
            });

        self.model.forward_search = search;
        self.model.forward_chosen = chosen;
        if send {
            self.send(Command::Forward {
                chats: self.model.forward_chosen.clone(),
                id: target.id.clone(),
            });
            self.model.forward_target = None;
        } else if close {
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
            self.release_video();
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
            PHOTO_PX,
        );
        let thumbs: Vec<(String, Option<egui::TextureHandle>)> = items
            .iter()
            .map(|entry| {
                let texture = texture_at(
                    ctx,
                    &mut self.textures,
                    &entry.id,
                    self.model.media_path(&entry.id),
                    PHOTO_PX,
                );
                (entry.id.clone(), texture)
            })
            .collect();

        let screen = ctx.screen_rect();
        // Paint at the clip's own rate: a 30 fps clip sampled by a 10 fps timer
        // looks like it stutters no matter how well the frames are decoded.
        if item.kind == "video" && self.video_playing {
            let fps = match self.videos.get(&item.id) {
                Some(VideoState::Ready(video)) => video.fps,
                _ => 30.0,
            };
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(1.0 / fps.max(1.0)));
        }

        let avatar = self.avatar_for(&item.chat).cloned();
        let chat_title = self
            .model
            .chats
            .get(&item.chat)
            .map(|row| self.model.row_title(row))
            .unwrap_or(chat_name.clone());
        let when = format!("Today at {}", stamp);
        let items_len = items.len();
        let selected = index;
        let mut open: Option<String> = None;
        let mut close = false;
        let mut toggle = false;
        let mut seek_to: Option<f32> = None;
        let mut toggle_mute = false;
        let mut starred = false;
        let mut save = false;
        let mut nav: Option<bool> = None;
        let description = item.text.clone();
        let note = self.note.clone();
        let kind = item.kind.clone();
        let item_id = item.id.clone();

        egui::Area::new(egui::Id::new("media-viewer"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let painter = ui.painter().clone();
                let backdrop = egui::Color32::from_rgb(11, 20, 26);
                painter.rect_filled(screen, 0.0, backdrop);

                // ---- header: who sent it, and the toolbar -----------------
                let header = 58.0;
                let avatar_rect = egui::Rect::from_min_size(
                    egui::pos2(screen.left() + 20.0, screen.top() + 12.0),
                    egui::vec2(33.0, 33.0),
                );
                if let Some(texture) = &avatar {
                    painter.image(
                        texture.id(),
                        avatar_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    painter.circle_filled(avatar_rect.center(), 16.5, avatar_color(&chat_title));
                    painter.text(
                        avatar_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        initials(&chat_title),
                        egui::FontId::proportional(13.0),
                        egui::Color32::WHITE,
                    );
                }
                painter.text(
                    egui::pos2(screen.left() + 62.0, screen.top() + 12.0),
                    egui::Align2::LEFT_TOP,
                    fit_text(
                        &painter,
                        &chat_title,
                        egui::FontId::proportional(14.5),
                        egui::Color32::from_gray(238),
                        (screen.width() * 0.4).max(120.0),
                    ),
                    egui::FontId::proportional(14.5),
                    egui::Color32::from_gray(238),
                );
                painter.text(
                    egui::pos2(screen.left() + 62.0, screen.top() + 30.0),
                    egui::Align2::LEFT_TOP,
                    &when,
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_gray(150),
                );

                // The toolbar, right to left: close, menu, forward, pin, star,
                // reply - the official order.
                let mut icons: Vec<(Icon, &str)> = vec![
                    (Icon::Close, "close"),
                    (Icon::Menu, "menu"),
                    (Icon::Forward, "forward"),
                    (Icon::Star, "star"),
                    (Icon::Reply, "reply"),
                ];
                icons.push((Icon::Download, "download"));
                let mut x = screen.right() - 18.0;
                for (icon, name) in icons {
                    let rect = egui::Rect::from_center_size(
                        egui::pos2(x - 17.0, screen.top() + header / 2.0),
                        egui::vec2(34.0, 34.0),
                    );
                    let response = ui.interact(rect, ui.id().with(name), egui::Sense::click());
                    let color = if response.hovered() {
                        egui::Color32::from_gray(245)
                    } else {
                        egui::Color32::from_gray(200)
                    };
                    if response.hovered() {
                        painter.rect_filled(
                            rect,
                            egui::CornerRadius::same(17),
                            egui::Color32::from_white_alpha(22),
                        );
                    }
                    draw_icon(ui, rect.shrink(8.0), icon, color);
                    if response.clicked() {
                        match name {
                            "close" => close = true,
                            "star" => starred = true,
                            "forward" => open = Some(String::new()),
                            "download" => save = true,
                            _ => {}
                        }
                    }
                    x -= 38.0;
                }

                // ---- the media itself ------------------------------------
                let strip = 74.0;
                let counter_row = 26.0;
                let stage = egui::Rect::from_min_max(
                    egui::pos2(screen.left() + 24.0, screen.top() + header),
                    egui::pos2(
                        screen.right() - 24.0,
                        screen.bottom() - strip - counter_row - 12.0,
                    ),
                );
                let mut media_rect = stage;
                if kind == "video" {
                    // The transport bar is an overlay inside the media, so the
                    // stage is measured to the frame, not around a separate row.
                    match self.video_frame_handle(ctx, &item_id) {
                        Some(frame) => {
                            let size = frame.size_vec2();
                            let scale = (stage.width() / size.x)
                                .min(stage.height() / size.y)
                                .max(0.01);
                            let target = egui::Rect::from_center_size(stage.center(), size * scale);
                            media_rect = target;
                            painter.image(
                                frame.id(),
                                target,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                            if !self.video_playing {
                                painter.rect_filled(
                                    target,
                                    egui::CornerRadius::same(2),
                                    egui::Color32::from_black_alpha(40),
                                );
                            }
                        }
                        None => {
                            let message = match self.videos.get(&item_id) {
                                Some(VideoState::Failed(err)) => format!("Cannot play: {err}"),
                                Some(VideoState::Ready(_)) => "Loading frame.".to_string(),
                                _ => "Preparing video.".to_string(),
                            };
                            painter.text(
                                stage.center(),
                                egui::Align2::CENTER_CENTER,
                                message,
                                egui::FontId::proportional(14.0),
                                egui::Color32::from_gray(190),
                            );
                        }
                    }
                } else {
                    match &texture {
                        Some(texture) => {
                            let size = texture.size_vec2();
                            let scale = (stage.width() / size.x)
                                .min(stage.height() / size.y)
                                .clamp(0.01, 1.0);
                            let target = egui::Rect::from_center_size(stage.center(), size * scale);
                            media_rect = target;
                            painter.image(
                                texture.id(),
                                target,
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
                                "Loading.",
                                egui::FontId::proportional(14.0),
                                egui::Color32::from_gray(190),
                            );
                        }
                    }
                }

                // Previous / next arrows in the gutters, like the official
                // viewer (they only appear when the chat has more than one).
                if items_len > 1 {
                    for (left, icon, step, name) in [
                        (true, Icon::ChevronLeft, false, "prev-media"),
                        (false, Icon::ChevronRight, true, "next-media"),
                    ] {
                        let x = if left {
                            stage.left() + 20.0
                        } else {
                            stage.right() - 20.0
                        };
                        let rect = egui::Rect::from_center_size(
                            egui::pos2(x, stage.center().y),
                            egui::vec2(40.0, 40.0),
                        );
                        let response = ui.interact(rect, ui.id().with(name), egui::Sense::click());
                        let fill = if response.hovered() {
                            egui::Color32::from_black_alpha(150)
                        } else {
                            egui::Color32::from_black_alpha(90)
                        };
                        painter.circle_filled(rect.center(), 20.0, fill);
                        draw_icon(ui, rect.shrink(10.0), icon, egui::Color32::from_gray(235));
                        if response.clicked() {
                            nav = Some(step);
                        }
                    }
                }

                // Clicking the frame plays or pauses, like the official viewer.
                let hit = ui.interact(
                    media_rect,
                    ui.id().with("media-stage"),
                    egui::Sense::click(),
                );
                if hit.clicked() && kind == "video" {
                    toggle = true;
                }

                // ---- video transport overlay ------------------------------
                if kind == "video" {
                    let bar = egui::Rect::from_min_max(
                        egui::pos2(media_rect.left(), media_rect.bottom() - 46.0),
                        media_rect.right_bottom(),
                    );
                    painter.rect_filled(
                        bar,
                        egui::CornerRadius {
                            nw: 0,
                            ne: 0,
                            sw: 2,
                            se: 2,
                        },
                        egui::Color32::from_black_alpha(90),
                    );
                    let middle = bar.center().y;
                    let play_rect = egui::Rect::from_center_size(
                        egui::pos2(bar.left() + 27.0, middle),
                        egui::vec2(34.0, 34.0),
                    );
                    let play = ui.interact(play_rect, ui.id().with("v-play"), egui::Sense::click());
                    draw_icon(
                        ui,
                        play_rect.shrink(7.0),
                        if self.video_playing {
                            Icon::Pause
                        } else {
                            Icon::Play
                        },
                        egui::Color32::from_gray(240),
                    );
                    if play.clicked() {
                        toggle = true;
                    }

                    let duration = match self.videos.get(&item_id) {
                        Some(VideoState::Ready(video)) => video.duration,
                        _ => 0.0,
                    };
                    let elapsed = self.video_elapsed().clamp(0.0, duration.max(0.0));
                    let seek_rect = egui::Rect::from_min_max(
                        egui::pos2(bar.left() + 52.0, middle - 9.0),
                        egui::pos2(bar.right() - 130.0, middle + 9.0),
                    );
                    let response = ui.interact(
                        seek_rect,
                        ui.id().with("v-seek"),
                        egui::Sense::click_and_drag(),
                    );
                    let line_y = middle;
                    painter.line_segment(
                        [
                            egui::pos2(seek_rect.left(), line_y),
                            egui::pos2(seek_rect.right(), line_y),
                        ],
                        egui::Stroke::new(3.0_f32, egui::Color32::from_white_alpha(70)),
                    );
                    let ratio = if duration > 0.0 {
                        (elapsed / duration).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let head = seek_rect.left() + seek_rect.width() * ratio;
                    painter.line_segment(
                        [
                            egui::pos2(seek_rect.left(), line_y),
                            egui::pos2(head, line_y),
                        ],
                        egui::Stroke::new(3.0_f32, egui::Color32::from_gray(240)),
                    );
                    painter.circle_filled(
                        egui::pos2(head, line_y),
                        6.0,
                        egui::Color32::from_gray(245),
                    );
                    if (response.clicked() || response.dragged())
                        && let Some(pos) = response.interact_pointer_pos()
                    {
                        let ratio =
                            ((pos.x - seek_rect.left()) / seek_rect.width()).clamp(0.0, 1.0);
                        seek_to = Some(ratio * duration);
                    }
                    painter.text(
                        egui::pos2(bar.right() - 120.0, middle),
                        egui::Align2::LEFT_CENTER,
                        "1.0×",
                        egui::FontId::proportional(12.5),
                        egui::Color32::from_gray(230),
                    );
                    let mute_rect = egui::Rect::from_center_size(
                        egui::pos2(bar.right() - 74.0, middle),
                        egui::vec2(28.0, 28.0),
                    );
                    let mute = ui.interact(mute_rect, ui.id().with("v-mute"), egui::Sense::click());
                    draw_icon(
                        ui,
                        mute_rect.shrink(6.0),
                        if self.video_muted {
                            Icon::Mute
                        } else {
                            Icon::Volume
                        },
                        egui::Color32::from_gray(235),
                    );
                    if mute.clicked() {
                        toggle_mute = true;
                    }
                    let full_rect = egui::Rect::from_center_size(
                        egui::pos2(bar.right() - 40.0, middle),
                        egui::vec2(28.0, 28.0),
                    );
                    draw_icon(
                        ui,
                        full_rect.shrink(6.0),
                        Icon::Fullscreen,
                        egui::Color32::from_gray(235),
                    );
                }

                // ---- caption, counter and the filmstrip -------------------
                let caption_y = stage.bottom() + 2.0;
                if !description.trim().is_empty() {
                    painter.text(
                        egui::pos2(media_rect.left(), caption_y),
                        egui::Align2::LEFT_TOP,
                        fit_text(
                            &painter,
                            &description,
                            egui::FontId::proportional(13.0),
                            egui::Color32::from_gray(200),
                            media_rect.width(),
                        ),
                        egui::FontId::proportional(13.0),
                        egui::Color32::from_gray(200),
                    );
                }
                let counter = format!("{} of {}", selected + 1, items_len.max(1));
                let counter_y = screen.bottom() - strip - counter_row + 2.0;
                painter.text(
                    egui::pos2(screen.center().x, counter_y),
                    egui::Align2::CENTER_TOP,
                    counter,
                    egui::FontId::proportional(12.5),
                    egui::Color32::from_gray(180),
                );

                // The strip: 64 px rounded thumbnails, the open one ringed green.
                let thumb_side = 58.0;
                let gap = 4.0;
                let total = items_len as f32 * (thumb_side + gap) - gap;
                let mut thumb_x = (screen.center().x - total / 2.0)
                    .max(screen.left() + 8.0)
                    .min(screen.right() - total - 8.0);
                let thumb_y = screen.bottom() - strip + 6.0;
                for (position, (id, thumb)) in thumbs.iter().enumerate() {
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(thumb_x, thumb_y),
                        egui::vec2(thumb_side, 62.0),
                    );
                    thumb_x += thumb_side + gap;
                    match thumb {
                        Some(texture) => {
                            let size = texture.size_vec2();
                            let scale = (thumb_side / size.x).min(62.0 / size.y);
                            let fitted = egui::Rect::from_center_size(rect.center(), size * scale);
                            painter.image(
                                texture.id(),
                                fitted,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        }
                        None => {
                            painter.rect_filled(rect, egui::CornerRadius::same(4), palette_thumb());
                        }
                    }
                    if position == selected {
                        painter.rect_stroke(
                            rect.expand(1.0),
                            egui::CornerRadius::same(4),
                            egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(0, 168, 132)),
                            egui::StrokeKind::Outside,
                        );
                    }
                    let response = ui.interact(
                        rect,
                        ui.id().with(("thumb", position)),
                        egui::Sense::click(),
                    );
                    if response.clicked() && position != selected {
                        open = Some(id.clone());
                    }
                }

                if !note.is_empty() {
                    painter.text(
                        egui::pos2(screen.right() - 24.0, screen.bottom() - 18.0),
                        egui::Align2::RIGHT_BOTTOM,
                        note,
                        egui::FontId::proportional(11.5),
                        egui::Color32::from_gray(170),
                    );
                }
            });

        if close {
            self.stop_video();
            self.release_video();
            self.model.close_viewer();
            return;
        }
        if starred {
            self.model.toggle_star(&item.id);
        }
        if save {
            self.note = match self.model.save_media(&item.id) {
                Ok(path) => format!("Saved to {path}"),
                Err(err) => err,
            };
        }
        if open == Some(String::new()) {
            self.model.forward_target = Some(item.clone());
            self.model.forward_search.clear();
            self.model.forward_chosen.clear();
        } else if let Some(id) = open {
            self.stop_video();
            self.model.open_viewer(&id);
        }
        if let Some(step) = nav {
            self.stop_video();
            self.model.step_viewer(step);
            return;
        }
        if toggle {
            self.toggle_video(&item.id);
        }
        if toggle_mute {
            self.video_muted = !self.video_muted;
            self.audio
                .set_volume(if self.video_muted { 0.0 } else { 1.0 });
        }
        if let Some(seconds) = seek_to {
            self.seek_video(&item.id, seconds);
        }
    }
}

/// The placeholder fill behind a filmstrip cell that has no thumbnail yet.
fn palette_thumb() -> egui::Color32 {
    egui::Color32::from_rgb(38, 42, 44)
}

/// The icons the window draws itself. Emoji glyphs are not in egui's bundled
/// fonts (they render as boxes), so every icon is a few strokes instead.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Icon {
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
    ChevronLeft,
    ChevronRight,
    Phone,
    CallEnd,
    VideoCall,
    Mic,
    SelectChats,
    MarkRead,
    Lock,
    Check,
}

/// The forward picker's square checkbox: an outlined box when it is clear, the
/// accent fill with a dark tick when it is set.
fn forward_checkbox(
    ui: &egui::Ui,
    palette: &crate::theme::Palette,
    left: f32,
    center_y: f32,
    checked: bool,
) {
    let box_rect =
        egui::Rect::from_center_size(egui::pos2(left + 10.0, center_y), egui::vec2(20.0, 20.0));
    if checked {
        ui.painter()
            .rect_filled(box_rect, egui::CornerRadius::same(5), palette.accent);
        draw_icon(
            ui,
            box_rect.shrink(3.0),
            Icon::Check,
            egui::Color32::from_rgb(11, 20, 26),
        );
    } else {
        ui.painter().rect_stroke(
            box_rect,
            egui::CornerRadius::same(5),
            egui::Stroke::new(1.5_f32, palette.secondary),
            egui::StrokeKind::Inside,
        );
    }
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

/// Paint one Material Symbol inside `rect`; the texture is cached per colour.
pub(crate) fn draw_icon(ui: &egui::Ui, rect: egui::Rect, icon: Icon, color: egui::Color32) {
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
            Icon::ChevronLeft => "chevron_left",
            Icon::ChevronRight => "chevron_right",
            Icon::Phone => "call",
            Icon::CallEnd => "call_end",
            Icon::VideoCall => "videocam",
            Icon::Mic => "mic",
            Icon::SelectChats => "done_all",
            Icon::MarkRead => "mark_chat_read",
            Icon::Lock => "lock",
            Icon::Check => "check",
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

/// The official `--WDS-components-profile-photo-surface-*` palette, used for
/// sender names and avatar fallbacks.
pub(crate) const COLORS: [u32; 8] = [
    0x53BDEB, 0x21C063, 0xF5A524, 0xE17E7E, 0xB39DDB, 0x7FD1C1, 0xEBA36A, 0x9FC7E8,
];

/// Group sender names are coloured per sender, like the official client.
pub(crate) fn sender_color(name: &str, palette: &crate::theme::Palette) -> egui::Color32 {
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
    // Official rows are rounded 12 px cards inset from the panel edge.
    let card = rect.shrink2(egui::vec2(6.0, 0.0));
    if selected {
        painter.rect_filled(card, egui::CornerRadius::same(12), palette.selected);
    } else if response.hovered() {
        painter.rect_filled(card, egui::CornerRadius::same(12), palette.hover);
    }

    let center = egui::pos2(rect.left() + 40.0, rect.center().y);
    // The `~` mark and a number are for the text only: the disc keeps the
    // person's initials.
    let avatar_name = title.trim_start_matches('~').trim();
    if let Some(texture) = texture {
        painter.image(
            texture.id(),
            egui::Rect::from_center_size(center, egui::vec2(48.0, 48.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        painter.circle_filled(center, 24.0, avatar_color(avatar_name));
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            initials(avatar_name),
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
            &single_line(title),
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
            &single_line(preview),
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

#[cfg(test)]
mod texture_budget_tests {
    use super::*;

    /// The caches have a hard ceiling: every extra texture evicts the oldest
    /// one that was not painted in the current frame.
    #[test]
    fn the_texture_cache_drops_the_least_recently_painted_entries() {
        let ctx = egui::Context::default();
        let mut cache = std::collections::HashMap::new();
        let cap = 8;
        for index in 0..40 {
            let key = format!("photo-{index}");
            let image = egui::ColorImage::filled([2, 2], egui::Color32::WHITE);
            let texture = ctx.load_texture(key.clone(), image, egui::TextureOptions::LINEAR);
            cache.insert(key, Some(texture));
            prune_textures(&ctx, &mut cache, cap);
            assert!(
                cache.len() <= cap,
                "cache grew to {} entries at {index}",
                cache.len()
            );
        }
        // The newest entry always survives.
        assert!(cache.contains_key("photo-39"));
        assert!(!cache.contains_key("photo-0"));
    }

    /// Sticker and quote textures are decoded small: they paint at 192 px and
    /// 96 px, so the full 512 px sticker bitmap would be wasted memory.
    #[test]
    fn textures_decode_at_the_size_they_paint_at() {
        const { assert!(STICKER_PX < PHOTO_PX && QUOTE_PX < STICKER_PX) };
    }
}

#[cfg(test)]
mod font_tests {
    use super::*;
    use crate::rich_text::clean_text;
    use crate::rich_text::is_emoji_char;

    /// "𝕋𝕒𝕦𝕗𝕚𝕢" is Mathematical Alphanumeric Symbols, not emoji: it must go
    /// through the text font chain (where the symbol face carries it), never
    /// through the colour-emoji rasteriser, and `clean_text` must keep it.
    #[test]
    fn mathematical_letters_stay_text() {
        let title = "𝕋𝕒𝕦𝕗𝕚𝕢";
        assert_eq!(title.chars().count(), 6);
        for ch in title.chars() {
            assert!(
                (0x1D400..=0x1D7FF).contains(&(ch as u32)),
                "U+{:04X}",
                ch as u32
            );
            assert!(!is_emoji_char(ch));
        }
        assert_eq!(clean_text(title), title);
    }

    #[test]
    fn the_symbol_group_prefers_a_base_windows_face() {
        assert_eq!(
            SYMBOL_FONTS.first().copied(),
            Some("C:\\Windows\\Fonts\\seguisym.ttf")
        );
        #[cfg(windows)]
        assert!(
            std::path::Path::new(SYMBOL_FONTS[0]).exists(),
            "Segoe UI Symbol ships with stock Windows"
        );
    }

    /// Invisible formatting codepoints are dropped; real text is untouched.
    #[test]
    fn clean_text_only_drops_invisible_formatting() {
        assert_eq!(clean_text("a\u{2060}b\u{200D}c"), "abc");
        assert_eq!(clean_text("Halo, apa kabar?"), "Halo, apa kabar?");
        assert_eq!(clean_text("🙏🏻"), "🙏🏻");
    }
}

#[cfg(test)]
mod composer_tests {
    use super::*;

    /// The Enter key only sends while the composer owns the focus.
    #[test]
    fn enter_sends_only_from_the_composer() {
        assert!(enter_sends(true, true));
        assert!(!enter_sends(true, false), "a stray Enter must not send");
        assert!(!enter_sends(false, true));
    }
}
