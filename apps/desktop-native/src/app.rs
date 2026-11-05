//! The window: a chat list on the left, the open conversation on the right.
//!
//! Painting is all this module does. Every string it shows comes from
//! `crate::model`, which in turn asks `wa-ui`.

use std::sync::mpsc::Sender;

use tokio::sync::mpsc::UnboundedReceiver;
use wa_types::{MessageView, UiEvent};

use crate::login_ui::demo_qr_svg;
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
    pub(crate) model: Model,
    pub(crate) events: UnboundedReceiver<UiEvent>,
    pub(crate) commands: Sender<Command>,
    pub(crate) runtime: tokio::runtime::Handle,
    pub(crate) typing_sent: bool,
    /// The status panel is showing instead of a conversation.
    pub(crate) show_status: bool,
    /// The calls panel is showing instead of a conversation.
    pub(crate) show_calls: bool,
    /// The Calls drawer's search query.
    pub(crate) calls_query: String,
    /// The Channels drawer's search query.
    pub(crate) channels_query: String,
    /// The Communities drawer shows every group, not the first four.
    pub(crate) communities_all: bool,
    /// The Communities screen, and the community opened in it.
    pub(crate) show_communities: bool,
    /// The Media screen (the official Media/Docs/Links panel).
    pub(crate) show_media: bool,
    /// 0 = Media, 1 = Docs, 2 = Links.
    pub(crate) media_tab: usize,
    /// The Media panel's search text.
    pub(crate) media_search: String,
    pub(crate) media_search_open: bool,
    /// The status viewer a compact emoji strip is open.
    pub(crate) status_emoji_open: bool,
    /// The status viewer a sticker strip is open.
    pub(crate) status_sticker_open: bool,
    /// Video ids whose poster frame has already been requested.
    pub(crate) posters_asked: std::collections::HashSet<String>,
    /// The group description is expanded ("Read more").
    pub(crate) group_desc_open: bool,
    /// The member list shows everyone, not just the first ten.
    pub(crate) group_members_all: bool,
    /// The You pane search box.
    pub(crate) settings_search: String,
    pub(crate) settings_search_open: bool,
    pub(crate) communities_active: Option<String>,
    /// The voice note being recorded from the composer's microphone button.
    pub(crate) voice: Option<crate::voicenote::Recording>,
    /// The attach menu is open.
    pub(crate) show_attach: bool,
    /// Poll and contact-card drafts (the attach menu's two dialogs).
    pub(crate) poll_draft: Option<crate::app::PollDraft>,
    pub(crate) contact_draft: Option<crate::app::ContactDraft>,
    pub(crate) location_draft: Option<crate::app::LocationDraft>,
    /// The name being typed for a new custom list.
    pub(crate) list_draft: Option<String>,
    /// `WA_DESK_SEND_ONCE` has fired (a smoke test of the send path).
    pub(crate) smoke_sent: bool,
    /// `WA_DESK_SCROLL_TOP` parks the conversation at its first message.
    pub(crate) preview_scroll_top: bool,
    /// `WA_DESK_ENTER_TEST` frames left before a synthetic Enter is pushed.
    pub(crate) enter_test: u8,
    /// Text being typed into the share-status box, plus the chosen background
    /// (an index into `wa_ui::composer::BACKGROUNDS`) and font.
    pub(crate) status_compose: Option<String>,
    pub(crate) status_compose_background: usize,
    pub(crate) status_compose_font: usize,
    /// The settings screen is showing instead of a conversation.
    pub(crate) show_settings: bool,
    pub(crate) settings: NativeSettings,
    pub(crate) settings_path: std::path::PathBuf,
    /// The last save result, shown under the form.
    pub(crate) settings_note: String,
    /// The channels panel is showing instead of a conversation.
    pub(crate) show_channels: bool,
    /// Rasterised QR code, keyed by the SVG it came from so a rotating code
    /// is decoded once per rotation.
    pub(crate) qr: Option<(String, egui::TextureHandle)>,
    /// The login gate is showing the phone-number form instead of the QR.
    pub(crate) gate_phone: bool,
    /// Media download results, drained every frame.
    pub(crate) media_rx: std::sync::mpsc::Receiver<(String, Result<String, String>)>,
    /// Group metadata replies.
    pub(crate) group_rx:
        std::sync::mpsc::Receiver<(String, Result<wa_types::GroupInfoView, String>)>,
    /// The group info screen is open.
    pub(crate) show_group: bool,
    /// Decoded media textures, keyed by message id.
    pub(crate) textures: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    /// Decoded sticker textures, keyed by sticker id or favourite id.
    pub(crate) sticker_textures: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    /// Favourite-sticker replies.
    pub(crate) favorites_rx:
        std::sync::mpsc::Receiver<Result<Vec<wa_types::FavoriteStickerView>, String>>,
    /// The search hit the list was last scrolled to.
    pub(crate) scrolled_hit: i64,
    /// A short status line for the viewer (saved path, failures).
    pub(crate) note: String,
    /// A transient toast ("Use the contact screen to call"), a few seconds.
    pub(crate) notice: Option<(String, std::time::Instant)>,
    /// Self-update state and its worker replies.
    pub(crate) update: UpdateState,
    pub(crate) update_tx: std::sync::mpsc::Sender<UpdateMsg>,
    pub(crate) update_rx: std::sync::mpsc::Receiver<UpdateMsg>,
    /// The automatic check has already run once.
    pub(crate) update_checked: bool,
    /// The resolved WhatsApp palette for this frame.
    pub(crate) palette: crate::theme::Palette,
    /// The OS dark-mode preference, captured before we install our own visuals.
    pub(crate) system_dark: bool,
    /// `--screenshot <path>`: save one frame and quit (used to verify the UI).
    pub(crate) screenshot: Option<(std::path::PathBuf, std::time::Instant, bool)>,
    /// The restart cache for the chat list (`chats.json` beside the session).
    pub(crate) cache_path: std::path::PathBuf,
    pub(crate) cache_dirty: bool,
    pub(crate) cache_saved: std::time::Instant,
    /// Profile photos: jid -> circular texture (None once known to be missing).
    pub(crate) avatars: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    /// Photos already asked for.
    pub(crate) avatar_asked: std::collections::HashSet<String>,
    /// JIDs whose photo was read from the disk cache.
    pub(crate) avatar_disk: std::collections::HashSet<String>,
    /// `WA_DESK_OPEN_FIRST`: opened the first chat once (screenshot testing).
    pub(crate) auto_opened: bool,
    pub(crate) avatar_rx: std::sync::mpsc::Receiver<(String, Option<String>)>,
    pub(crate) avatar_img_tx: std::sync::mpsc::Sender<(String, Result<Vec<u8>, String>)>,
    pub(crate) avatar_img_rx: std::sync::mpsc::Receiver<(String, Result<Vec<u8>, String>)>,
    /// Attachment results, and the short note under the composer.
    pub(crate) attach_rx: std::sync::mpsc::Receiver<Result<String, String>>,
    pub(crate) attach_note: Option<(String, std::time::Instant)>,
    /// True in `--preview`: never write the demo chats to the real cache.
    pub(crate) preview: bool,
    /// When the last `memory:` line was written (diagnostics only).
    pub(crate) last_memory_log: std::time::Instant,
    /// Paint clock for the `paint:` diagnostic: the previous frame's time and
    /// the interval seen since the last report.
    pub(crate) last_paint: std::time::Instant,
    pub(crate) paint_gap_sum: f32,
    pub(crate) paint_gap_count: u32,
    /// Screenshot support: start the demo video as soon as it is decoded.
    pub(crate) preview_autoplay: bool,
    /// Progressive history sync: batches left and when the last one went out.
    pub(crate) sync_rounds: u8,
    pub(crate) last_sync: std::time::Instant,
    /// The "new chat" FAB asked the search box to take focus.
    pub(crate) focus_search: bool,
    /// The connection epoch the backfill already started for.
    pub(crate) seen_epoch: u64,
    /// When the chat app-state was last polled for read marks/mutes.
    pub(crate) last_state_sync: std::time::Instant,
    /// When groups/status were last refreshed in full.
    pub(crate) last_full_sync: std::time::Instant,
    /// Prepared videos and their frame textures.
    pub(crate) videos: std::collections::HashMap<String, VideoState>,
    pub(crate) video_tx: std::sync::mpsc::Sender<(String, Result<crate::video::Video, String>)>,
    pub(crate) video_rx: std::sync::mpsc::Receiver<(String, Result<crate::video::Video, String>)>,
    pub(crate) video_textures: std::collections::HashMap<String, Option<egui::TextureHandle>>,
    /// Insertion order of `video_textures`, so old frames can be evicted.
    pub(crate) video_order: std::collections::VecDeque<String>,
    /// The clip whose frames are being decoded ahead, keyed by its directory.
    pub(crate) video_prefetch: Option<(String, crate::frames::Prefetch)>,
    pub(crate) video_playing: bool,
    pub(crate) video_started: Option<std::time::Instant>,
    pub(crate) video_offset: f32,
    pub(crate) video_muted: bool,
    /// Group JIDs whose metadata has been requested.
    pub(crate) groups_asked: std::collections::HashSet<String>,
    /// The Chats kebab menu is open.
    pub(crate) show_chat_menu: bool,
    /// Which Settings section is open.
    pub(crate) settings_section: usize,
    /// The audio player, kept alive for the window's lifetime.
    pub(crate) audio: crate::audio::Audio,
    /// True once the Packs tab asked the session for its packs.
    pub(crate) packs_requested: bool,
    /// The sticker picker tab (0 recents, 1 favourites, 2 packs) and search.
    pub(crate) sticker_tab: usize,
    pub(crate) sticker_query: String,
    /// The sticker store sheet is open (the `+` in the panel's tab row).
    pub(crate) sticker_store: bool,
    /// The emoji picker's selected category (0 = recents, 1.. = catalogue groups).
    pub(crate) emoji_group: usize,
    /// The picker's GIF tab (no GIF search yet, so it explains itself).
    pub(crate) emoji_gif: bool,
    /// Colour emoji textures, keyed by (glyph, pixel size).
    pub(crate) emoji_textures: std::collections::HashMap<(char, u32), Option<egui::TextureHandle>>,
    /// The remote call video decoder, started on the first frame.
    pub(crate) call_video: Option<crate::callvideo::CallVideo>,
    /// The newest decoded remote frame, uploaded as a texture.
    pub(crate) call_frame: Option<egui::TextureHandle>,
    /// Preview hook: an image used as the self-view in `--preview`.
    pub(crate) call_self_source: Option<String>,
    /// Our own camera preview shown in the call sheet corner.
    pub(crate) call_self_frame: Option<egui::TextureHandle>,
    /// Which decoder generation call_frame holds.
    pub(crate) call_frame_generation: u64,
    /// The Windows tray icon and menu (None when the shell refuses it).
    pub(crate) tray: Option<crate::tray::Tray>,
    /// The open chat's actions menu.
    pub(crate) show_chat_actions: bool,
    /// The new-chat picker and its search.
    pub(crate) show_new_chat: bool,
    pub(crate) new_chat_search: String,
    pub(crate) contacts: Vec<(String, String)>,
    pub(crate) contacts_rx: std::sync::mpsc::Receiver<Vec<(String, String)>>,
    pub(crate) show_starred: bool,
    pub(crate) show_new_group: bool,
    pub(crate) new_group_subject: String,
    pub(crate) new_group_members: std::collections::HashSet<String>,
    pub(crate) show_new_contact: bool,
    pub(crate) new_contact_phone: String,
    pub(crate) new_contact_name: String,
    pub(crate) group_add_open: bool,
    pub(crate) group_add_members: std::collections::HashSet<String>,
}

impl WaDesk {
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
        // `WA_DESK_UPDATE_DEMO=<percent>` paints the Updates page mid-download,
        // so the button-shaped progress bar can be captured without waiting for
        // a release to actually download.
        // `WA_DESK_NO_CHAT=1` leaves the conversation empty, for the live
        // empty-state layout (promo card + quick actions).
        if std::env::var("WA_DESK_NO_CHAT").is_ok() {
            self.model.active = None;
        }
        if let Ok(percent) = std::env::var("WA_DESK_UPDATE_DEMO")
            && let Ok(value) = percent.parse::<f32>()
        {
            use std::sync::atomic::Ordering;
            let total: u64 = 27_169_792;
            let done = (total as f64 * (value.clamp(0.0, 100.0) as f64 / 100.0)) as u64;
            crate::updates::DOWNLOAD_TOTAL.store(total, Ordering::Relaxed);
            crate::updates::DOWNLOAD_DONE.store(done, Ordering::Relaxed);
            self.update = crate::updates::UpdateState::Downloading(crate::updates::Release {
                tag: "v0.2.1-rc.74".to_string(),
                url: String::new(),
                size: total,
                notes_url: String::new(),
            });
            self.show_settings = true;
            self.settings_section = 7;
        }
        // `WA_DESK_POLL=1` / `WA_DESK_CONTACT=1` put the two new message kinds in
        // the demo group so their bubbles can be captured.
        if let Ok(media) = std::env::var("WA_DESK_STATUS_MEDIA") {
            // A photo story with a real file registered as its media, so the
            // stage and its backdrop can be captured.
            let jid = "628111000111@s.whatsapp.net".to_string();
            let id = "status-media-demo".to_string();
            self.model.apply(&UiEvent::Status(wa_types::StatusUpdate {
                id: id.clone(),
                sender: jid.clone(),
                name: "Ada".to_string(),
                kind: "image".to_string(),
                text: "shot on the balcony".to_string(),
                background: None,
                timestamp: now_ms(),
                from_me: false,
            }));
            self.model
                .media
                .insert(id, crate::model::MediaState::Path(media));
            self.show_status = true;
            self.model.open_status(&jid);
            let index = self
                .model
                .status_groups()
                .iter()
                .position(|group| group.jid == jid)
                .unwrap_or(0);
            self.model.viewer = Some((index, 0));
        }
        if std::env::var("WA_DESK_STATUS_VIEWER").is_ok() {
            self.show_status = true;
            if let Some(first) = self.model.status_groups().first() {
                let jid = first.jid.clone();
                self.model.open_status(&jid);
            }
        }
        if std::env::var("WA_DESK_ENTER_TEST").is_ok() {
            self.model.composer = "Enter key sends this line".to_string();
            self.model.active = Some("1203@g.us".to_string());
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
        // `WA_DESK_STICKER_STORE=1` opens the sticker store sheet for a capture.
        if std::env::var("WA_DESK_STICKER_STORE").is_ok() {
            self.model.stickers_open = true;
            self.sticker_store = true;
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
        // `WA_DESK_VIEWER_IMAGE=<file>` opens the viewer on the demo photo, so
        // the still-image chrome can be captured.
        if let Ok(path) = std::env::var("WA_DESK_VIEWER_IMAGE") {
            self.model.media_ready("d10", &path);
            self.model.open_viewer("d10");
        }
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

    /// Load a WhatsApp "Export chat" file into the open chat.
    pub(crate) fn import_chat_export(&mut self, chat: &str) {
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

    /// Start or pause the open clip, keeping the extracted audio in step.
    pub(crate) fn toggle_video(&mut self, id: &str) {
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
    pub(crate) fn seek_video(&mut self, id: &str, seconds: f32) {
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

    /// Start a call in the open chat, with the official guard for groups and
    /// self-chat.
    pub(crate) fn start_call(&mut self, video: bool) {
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

    /// Check the release list on a worker thread; the window never blocks.
    pub(crate) fn check_updates(&mut self) {
        if update_demo_active() {
            return;
        }
        self.update = UpdateState::Checking;
        let tx = self.update_tx.clone();
        std::thread::spawn(move || {
            let reply = updates::check(env!("CARGO_PKG_VERSION"));
            let _ = tx.send(UpdateMsg::Checked(reply));
        });
    }

    /// Download the release binary on a worker thread.
    pub(crate) fn download_update(&mut self, release: Release) {
        self.update = UpdateState::Downloading(release.clone());
        let tx = self.update_tx.clone();
        std::thread::spawn(move || {
            let reply = updates::download(&release, &updates::staging_path());
            let _ = tx.send(UpdateMsg::Downloaded(reply));
        });
    }

    pub(crate) fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// The paperclip: pick a file and send it to the open chat.
    pub(crate) fn pick_attachment(&mut self) {
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
    pub(crate) fn send_document(
        &mut self,
        chat: &str,
        path: std::path::PathBuf,
        caption: Option<String>,
    ) {
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
    pub(crate) fn ensure_avatar(&mut self, ctx: &egui::Context, jid: &str) {
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

    pub(crate) fn avatar_for(&self, jid: &str) -> Option<&egui::TextureHandle> {
        self.avatars.get(jid).and_then(|entry| entry.as_ref())
    }

    /// Run what a bubble's context menu asked for.
    pub(crate) fn run_action(&mut self, row: &ChatRow, action: BubbleAction) {
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
}

/// Enter sends the message only while the composer has (or has just lost) the
/// keyboard focus: without that guard a stray Enter anywhere in the window -
/// the search field, a dialog - would post the draft.
pub(crate) fn enter_sends(enter_pressed: bool, composer_focused: bool) -> bool {
    enter_pressed && composer_focused
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

impl WaDesk {}

impl WaDesk {}

/// A story frame as a message, so `wa-ui`'s clock label can format it.
pub(crate) fn status_as_message(update: &wa_ui::status::StatusUpdateView) -> MessageView {
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

/// What the viewer knows about a video message.
#[derive(Debug, Clone)]
pub(crate) enum VideoState {
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

pub(crate) fn now_ms() -> i64 {
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

impl WaDesk {}

impl WaDesk {
    /// Settings: the official two-pane screen - the section list on the left,
    /// the selected page as cards on the right.
    pub(crate) fn settings_panel(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    self.settings_sections(ui, &palette);
                    ui.add_space(2.0);
                    ui.separator();
                    // The detail pane must scroll inside the window: without an
                    // explicit max height the scroll area grew to its content
                    // and the last card was clipped by the window edge.
                    // `available_height()` is not the window's remaining height
                    // inside this horizontal layout, so measure from the screen.
                    let pane_height =
                        (ui.ctx().screen_rect().bottom() - ui.cursor().top() - 4.0).max(140.0);
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                        )
                        .max_height(pane_height)
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
}

/// One category tab of the emoji picker: the group's own emoji on a pill, with
/// the accent underline the official panel uses for the active tab.
pub(crate) fn emoji_category(
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
pub(crate) fn notification_body(kind: &str, text: &str) -> String {
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
pub(crate) fn unread_divider(ui: &mut egui::Ui, palette: &crate::theme::Palette) {
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

// The moved helpers keep their old paths, so the call sites stay untouched.
pub(crate) use crate::buttons::{
    action_button, fab_button, pill_button, pill_plus_button, rail_button,
};
pub(crate) use crate::fonts::install_fonts;
pub(crate) use crate::icon_widgets::{Icon, draw_icon, forward_checkbox, icon_button};
/// The official `--WDS-components-profile-photo-surface-*` palette, used for
/// sender names and avatar fallbacks.
pub(crate) const COLORS: [u32; 8] = [
    0x53BDEB, 0x21C063, 0xF5A524, 0xE17E7E, 0xB39DDB, 0x7FD1C1, 0xEBA36A, 0x9FC7E8,
];

pub(crate) use crate::paint::{
    circular_avatar, palette_thumb, prune_textures, sender_color, thumb_texture,
};
pub(crate) use crate::wallpaper::paint_wallpaper;

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

/// One chat row: avatar, name, preview, clock and the unread pill.
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
        const {
            assert!(
                crate::bubbles::STICKER_PX < crate::bubbles::PHOTO_PX
                    && crate::bubbles::QUOTE_PX < crate::bubbles::STICKER_PX
            )
        };
    }
}

#[cfg(test)]
mod font_tests {
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
            crate::fonts::SYMBOL_FONTS.first().copied(),
            Some("C:\\Windows\\Fonts\\seguisym.ttf")
        );
        #[cfg(windows)]
        assert!(
            std::path::Path::new(crate::fonts::SYMBOL_FONTS[0]).exists(),
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

/// `WA_DESK_UPDATE_DEMO` forces the Updates page into its downloading state, so
/// the launch check must not overwrite it with the real "up to date" result.
pub(crate) fn update_demo_active() -> bool {
    std::env::var_os("WA_DESK_UPDATE_DEMO").is_some()
}
