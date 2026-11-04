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
    /// Progressive history sync: batches left and when the last one went out.
    sync_rounds: u8,
    last_sync: std::time::Instant,
    /// The "new chat" FAB asked the search box to take focus.
    focus_search: bool,
    /// The connection epoch the backfill already started for.
    seen_epoch: u64,
    /// When the chat app-state was last polled for read marks/mutes.
    last_state_sync: std::time::Instant,
    /// Group JIDs whose metadata has been requested.
    groups_asked: std::collections::HashSet<String>,
    /// The Chats kebab menu is open.
    show_chat_menu: bool,
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
            auto_opened: false,
            avatar_rx: native.avatars,
            avatar_img_tx,
            avatar_img_rx,
            attach_rx: native.attach,
            attach_note: None,
            preview: false,
            sync_rounds: 6,
            last_sync: std::time::Instant::now(),
            focus_search: false,
            seen_epoch: 0,
            last_state_sync: std::time::Instant::now(),
            groups_asked: std::collections::HashSet::new(),
            show_chat_menu: std::env::var("WA_DESK_MENU").is_ok(),
        }
    }

    /// Fill the model with sample chats so the UI can be inspected without a
    /// session (`--preview`, used for screenshots).
    pub fn preview_mode(&mut self) {
        self.model = Model::demo();
        self.preview = true;
        self.show_group = false;
        self.show_settings = false;
        self.show_status = false;
        self.show_channels = false;
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
                ui.set_min_width(220.0);
                if menu_row(ui, &palette, Icon::Settings, "Settings") {
                    self.show_settings = true;
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::Document, "Sync chat history") {
                    self.send(Command::InitialSync);
                    self.send(Command::SyncChatState);
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::Groups, "Sync groups") {
                    self.send(Command::SyncGroups);
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::Status, "Refresh statuses") {
                    self.send(Command::StatusHistory);
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::Logout, "Log out") {
                    self.send(Command::Relink);
                    self.show_chat_menu = false;
                }
            });
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
            self.model.apply(&event);
            changed = true;
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
        while let Ok(result) = self.attach_rx.try_recv() {
            let note = match result {
                Ok(note) => note,
                Err(err) => err,
            };
            self.attach_note = Some((note, std::time::Instant::now()));
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
    fn ensure_avatar(&mut self, jid: &str) {
        if jid.is_empty() || self.avatar_asked.contains(jid) {
            return;
        }
        self.avatar_asked.insert(jid.to_string());
        self.send(Command::Avatar {
            jid: jid.to_string(),
        });
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
                    self.ensure_avatar(&self.model.me_jid.clone());
                    let own_avatar = self.avatar_for(&self.model.me_jid.clone()).cloned();
                    paint_avatar(ui, &self.model.own_name(), 30.0, own_avatar.as_ref());
                });
            });

        egui::SidePanel::left("chats")
            .resizable(true)
            .default_width(368.0)
            .show(ctx, |ui| {
                let palette = self.palette.clone();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    self.ensure_avatar(&self.model.me_jid.clone());
                    let own_avatar = self.avatar_for(&self.model.me_jid.clone()).cloned();
                    paint_avatar(ui, &self.model.own_name(), 34.0, own_avatar.as_ref());
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("Chats").size(19.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if fab_button(ui, &palette).on_hover_text("New chat").clicked() {
                            self.focus_search = true;
                        }
                        if icon_button(ui, Icon::Menu, &palette)
                            .on_hover_text("Menu")
                            .clicked()
                        {
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
                            self.ensure_avatar(&row.jid);
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
                            self.ensure_avatar(&row.jid);
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
                                    if row.jid.ends_with("@g.us")
                                        && icon_button(ui, Icon::Menu, &palette)
                                            .on_hover_text("Group info")
                                            .clicked()
                                    {
                                        self.show_group = true;
                                        if self.model.request_group(&row.jid) {
                                            self.send(Command::GroupInfo {
                                                chat: row.jid.clone(),
                                            });
                                        }
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
                // request each). The same sticker files feed the picker.
                let wanted: Vec<String> = row
                    .messages
                    .iter()
                    .filter(|message| matches!(message.kind.as_str(), "image" | "sticker"))
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
                egui::ScrollArea::vertical()
                    .stick_to_bottom(focus.is_none())
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
        let time = ui.painter().layout_no_wrap(
            model.bubble_stamp(message),
            egui::FontId::proportional(11.0),
            palette.secondary,
        );
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
        let width = (text.size().x.max(time.size().x + 34.0) + padding * 2.0).min(max_width);
        let mut height = text.size().y + time.size().y + 8.0;
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
        painter.galley(egui::pos2(rect.left() + padding, y), text, palette.text);
        painter.galley(
            egui::pos2(
                rect.right() - padding - time.size().x,
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
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
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

/// Add the OS emoji font as a fallback: egui's bundled NotoEmoji subset draws
/// many common emoji as empty boxes.
fn install_fonts(ctx: &egui::Context) {
    let candidates = [
        // Windows: Segoe UI Emoji (outlines are readable even without colour).
        "C:\\Windows\\Fonts\\seguiemj.ttf",
        "C:\\Windows\\Fonts\\seguisym.ttf",
        // Linux.
        "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
        "/usr/share/fonts/noto/NotoColorEmoji.ttf",
        "/usr/share/fonts/truetype/noto/NotoEmoji-Regular.ttf",
    ];
    let Some(bytes) = candidates.iter().find_map(|path| std::fs::read(path).ok()) else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "system-emoji".to_owned(),
        std::sync::Arc::new(egui::FontData::from_owned(bytes)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("system-emoji".to_owned());
    }
    ctx.set_fonts(fonts);
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
    /// Settings: theme and the AI form, validated by wa-ui.
    fn settings_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Settings");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Back to chats").clicked() {
                        self.show_settings = false;
                    }
                });
            });
            ui.separator();

            ui.label(egui::RichText::new("Theme").strong());
            let system_dark = ctx.style().visuals.dark_mode;
            ui.horizontal(|ui| {
                for choice in ["light", "dark", "system"] {
                    let label = wa_ui::settings::theme_label(choice);
                    if ui
                        .selectable_label(self.settings.theme == choice, label)
                        .clicked()
                    {
                        self.settings.theme = choice.to_string();
                    }
                }
            });
            ui.label(
                egui::RichText::new(format!(
                    "{} · showing {}",
                    self.settings.theme_label(),
                    self.settings.resolved_theme(system_dark)
                ))
                .small()
                .weak(),
            );

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Chat history").strong());
            ui.label(
                egui::RichText::new(
                    "The phone only pushes the full chat list to a newly linked device; this \
                     asks for a batch from the linked session (eight chats at a time). For the \
                     complete history, unlink and pair again.",
                )
                .small()
                .weak(),
            );
            if ui.button("Sync chat history").clicked() {
                self.send(Command::InitialSync);
                self.settings_note = "Asking the phone for history…".to_string();
            }
            if ui.button("Sync more chats").clicked() {
                self.send(Command::SyncMore);
                self.settings_note = "Asking for the next chats…".to_string();
            }
            if ui.button("Refresh statuses").clicked() {
                self.send(Command::StatusHistory);
                self.settings_note = "Asking the phone for recent statuses…".to_string();
            }
            if ui
                .button("Unlink and pair again (imports the full list)")
                .clicked()
            {
                self.send(Command::Relink);
                self.settings_note =
                    "Unlinked - scan the QR with the phone to import the full chat list."
                        .to_string();
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Updates").strong());
            ui.label(
                egui::RichText::new(format!(
                    "Installed v{} · published on GitHub releases",
                    env!("CARGO_PKG_VERSION")
                ))
                .small()
                .weak(),
            );
            let state = self.update.clone();
            let busy = matches!(state, UpdateState::Checking | UpdateState::Downloading(_));
            ui.horizontal(|ui| {
                if !busy && ui.button("Check for updates").clicked() {
                    self.check_updates();
                }
                match state {
                    UpdateState::Idle => {}
                    UpdateState::Checking => {
                        ui.label(egui::RichText::new("Checking…").small().weak());
                    }
                    UpdateState::UpToDate => {
                        ui.label(
                            egui::RichText::new("You are on the newest release.")
                                .small()
                                .weak(),
                        );
                    }
                    UpdateState::Available(release) => {
                        let megabytes = release.size as f64 / 1_048_576.0;
                        ui.label(egui::RichText::new(format!(
                            "{} is available ({megabytes:.1} MB)",
                            release.tag
                        )));
                        if !release.notes_url.is_empty() {
                            ui.hyperlink_to("notes", &release.notes_url);
                        }
                        if ui.button("Download & install").clicked() {
                            self.download_update(release);
                        }
                    }
                    UpdateState::Downloading(release) => {
                        ui.label(
                            egui::RichText::new(format!("Downloading {}…", release.tag))
                                .small()
                                .weak(),
                        );
                    }
                    UpdateState::Ready(path) => {
                        if ui.button("Restart now").clicked() {
                            match updates::install(&path) {
                                Ok(()) => std::process::exit(0),
                                Err(err) => self.update = UpdateState::Failed(err),
                            }
                        }
                        ui.label(
                            egui::RichText::new("Downloaded — restarting finishes the update.")
                                .small()
                                .weak(),
                        );
                    }
                    UpdateState::Failed(err) => {
                        ui.label(
                            egui::RichText::new(err)
                                .small()
                                .color(egui::Color32::from_rgb(251, 80, 97)),
                        );
                    }
                }
            });

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(egui::RichText::new("AI assistant").strong());
            ui.label(egui::RichText::new(self.settings.key_hint()).small().weak());
            ui.checkbox(&mut self.settings.ai.enabled, "AI mode");
            ui.checkbox(&mut self.settings.ai.require_trigger, "Require /ai trigger");

            egui::Grid::new("ai-form").num_columns(2).show(ui, |ui| {
                ui.label("Base URL");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.ai.base_url).desired_width(380.0),
                );
                ui.end_row();

                ui.label("Model");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.ai.model).desired_width(380.0),
                );
                ui.end_row();

                ui.label("Vision model");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.ai.vision_model)
                        .desired_width(380.0),
                );
                ui.end_row();

                ui.label("System prompt");
                ui.add(
                    egui::TextEdit::multiline(&mut self.settings.ai.system_prompt)
                        .desired_width(380.0)
                        .desired_rows(3),
                );
                ui.end_row();

                ui.label("API key");
                ui.add(
                    egui::TextEdit::singleline(&mut self.settings.ai.api_key)
                        .password(true)
                        .hint_text("leave blank to keep the stored key")
                        .desired_width(380.0),
                );
                ui.end_row();
            });

            ui.add_space(8.0);
            if ui.button("Save").clicked() {
                let plan = self.settings.ai_plan();
                if !plan.ok {
                    self.settings_note = plan.error.unwrap_or_default();
                } else {
                    match self.settings.save(&self.settings_path) {
                        Ok(()) => {
                            if !self.settings.ai.api_key.is_empty() {
                                self.settings.ai.has_key = true;
                            }
                            self.settings.ai.api_key.clear();
                            self.settings_note = plan.toast;
                        }
                        Err(err) => self.settings_note = err,
                    }
                }
            }
            if !self.settings_note.is_empty() {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(&self.settings_note).small().weak());
            }
        });
    }
}

/// Where the native app keeps its own settings.
fn settings_file() -> std::path::PathBuf {
    let base = std::env::var("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|_| {
            std::env::var("HOME").map(|home| std::path::PathBuf::from(home).join(".local/share"))
        })
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let dir = base.join("io.github.dirttw.wadesk-native");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("settings.json")
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
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (name, admin, _jid) in self.model.group_members() {
                        ui.horizontal(|ui| {
                            ui.label(name);
                            if admin {
                                ui.label(
                                    egui::RichText::new("Admin")
                                        .small()
                                        .color(egui::Color32::from_rgb(37, 211, 102)),
                                );
                            }
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
        self.drain_events(ctx);
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
            if self.sync_rounds == 0 {
                self.sync_rounds = 6;
                self.last_sync = std::time::Instant::now();
            }
            self.send(Command::SyncChatState);
            self.send(Command::SyncGroups);
            self.last_state_sync = std::time::Instant::now();
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
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
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
    fn emoji_window(&mut self, ctx: &egui::Context) {
        if !self.model.emoji_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Emoji")
            .open(&mut open)
            .default_pos([420.0, 160.0])
            .default_width(320.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.model.emoji_query)
                        .hint_text("Search emoji")
                        .desired_width(f32::INFINITY),
                );
                let items = self.model.emoji_items();
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for item in items {
                                if ui
                                    .add(egui::Button::new(egui::RichText::new(&item).size(19.0)))
                                    .clicked()
                                {
                                    self.model.insert_emoji(&item);
                                }
                            }
                        });
                    });
            });
        if !open {
            self.model.emoji_open = false;
        }
    }

    /// The sticker picker: the account's favourites, then the stickers the
    /// session already downloaded from the chats. Clicking sends one.
    fn stickers_window(&mut self, ctx: &egui::Context) {
        if !self.model.stickers_open {
            return;
        }
        let mut open = true;
        let chat = self.model.active.clone().unwrap_or_default();
        let favorites = self.model.favorites.clone();
        let error = self.model.favorites_error.clone();
        let recent = self.model.sticker_collection();
        egui::Window::new("Stickers")
            .open(&mut open)
            .default_pos([420.0, 180.0])
            .default_width(340.0)
            .show(ctx, |ui| {
                if chat.is_empty() {
                    ui.label(egui::RichText::new("Open a chat to send a sticker").weak());
                    return;
                }
                if let Some(err) = error {
                    ui.label(
                        egui::RichText::new(format!("Favourites: {err}"))
                            .small()
                            .weak(),
                    );
                }
                if !favorites.is_empty() {
                    ui.label(egui::RichText::new("Favourites").small().strong());
                    ui.horizontal_wrapped(|ui| {
                        for favorite in &favorites {
                            let key = format!("fav:{}", favorite.id);
                            if let Some(texture) = texture_at(
                                ctx,
                                &mut self.sticker_textures,
                                &key,
                                Some(favorite.path.clone()),
                            ) {
                                let size = texture.size_vec2();
                                let scale = (64.0 / size.x).min(1.0);
                                if ui
                                    .add(
                                        egui::Image::new((texture.id(), size * scale))
                                            .sense(egui::Sense::click()),
                                    )
                                    .clicked()
                                {
                                    self.send(Command::SendFavoriteSticker {
                                        chat: chat.clone(),
                                        id: favorite.id.clone(),
                                    });
                                    self.model.stickers_open = false;
                                }
                            }
                        }
                    });
                    ui.separator();
                }
                ui.label(egui::RichText::new("Recent").small().strong());
                if recent.is_empty() {
                    ui.label(
                        egui::RichText::new("Send or receive a sticker and it shows up here")
                            .small()
                            .weak(),
                    );
                    return;
                }
                ui.horizontal_wrapped(|ui| {
                    for (id, path) in recent {
                        let key = format!("msg:{id}");
                        if let Some(texture) =
                            texture_at(ctx, &mut self.sticker_textures, &key, Some(path))
                        {
                            let size = texture.size_vec2();
                            let scale = (64.0 / size.x).min(1.0);
                            if ui
                                .add(
                                    egui::Image::new((texture.id(), size * scale))
                                        .sense(egui::Sense::click()),
                                )
                                .clicked()
                            {
                                self.send(Command::ForwardSticker {
                                    chat: chat.clone(),
                                    message_id: id.clone(),
                                });
                                self.model.stickers_open = false;
                            }
                        }
                    }
                });
            });
        if !open {
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
            self.model.close_viewer();
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            self.model.step_viewer(true);
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            self.model.step_viewer(false);
            return;
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
                                    if ui.button("✕").clicked() {
                                        self.model.close_viewer();
                                    }
                                    if ui.button("Save").clicked() {
                                        self.note = match self.model.save_media(&item.id) {
                                            Ok(path) => format!("Saved to {path}"),
                                            Err(err) => err,
                                        };
                                    }
                                    ui.label(egui::RichText::new(&counter).small().weak());
                                },
                            );
                        });

                        let stage_height = (ui.available_height() - 110.0).max(160.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), stage_height),
                            egui::Layout::centered_and_justified(egui::Direction::TopDown),
                            |ui| match &texture {
                                Some(texture) => {
                                    let size = texture.size_vec2();
                                    let scale = (ui.available_width() / size.x)
                                        .min(ui.available_height() / size.y)
                                        .min(1.0);
                                    ui.image((texture.id(), size * scale));
                                }
                                None => {
                                    ui.label(
                                        egui::RichText::new("Loading…")
                                            .color(egui::Color32::from_gray(200)),
                                    );
                                }
                            },
                        );

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
                                    if ui.button("◀").clicked() {
                                        self.model.step_viewer(false);
                                    }
                                    if ui.button("▶").clicked() {
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
    Document,
    Attach,
    Chats,
    Add,
    Groups,
    Logout,
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
            Icon::Document => "description",
            Icon::Attach => "attach_file",
            Icon::Chats => "chat_bubble",
            Icon::Add => "add",
            Icon::Groups => "groups",
            Icon::Logout => "logout",
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
