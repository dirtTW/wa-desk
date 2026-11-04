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
}

impl NativeApp {
    pub fn new(
        events: UnboundedReceiver<UiEvent>,
        commands: Sender<Command>,
        runtime: tokio::runtime::Handle,
        media: std::sync::mpsc::Receiver<(String, Result<String, String>)>,
        group: std::sync::mpsc::Receiver<(String, Result<wa_types::GroupInfoView, String>)>,
        favorites: std::sync::mpsc::Receiver<Result<Vec<wa_types::FavoriteStickerView>, String>>,
    ) -> Self {
        Self {
            events,
            commands,
            runtime,
            media,
            group,
            favorites,
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
}

impl WaDesk {
    pub fn new(_cc: &eframe::CreationContext<'_>, native: NativeApp) -> Self {
        let (update_tx, update_rx) = std::sync::mpsc::channel();
        Self {
            model: Model::new(),
            events: native.events,
            commands: native.commands,
            runtime: native.runtime,
            typing_sent: false,
            show_status: false,
            show_channels: false,
            show_settings: false,
            settings: NativeSettings::load(&settings_file()),
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
        }
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
        while let Ok((id, reply)) = self.media_rx.try_recv() {
            match reply {
                Ok(path) => self.model.media_ready(&id, &path),
                Err(_) => self.model.media_failed(&id),
            }
            changed = true;
        }
        if changed {
            ctx.request_repaint();
        }
    }

    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("chats")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.heading("Chats");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if matches!(
                            self.update,
                            UpdateState::Available(_) | UpdateState::Ready(_)
                        ) && ui
                            .button(
                                egui::RichText::new("⬆ Update")
                                    .color(egui::Color32::from_rgb(37, 211, 102)),
                            )
                            .clicked()
                        {
                            self.show_settings = true;
                            self.show_channels = false;
                            self.show_status = false;
                            self.model.close_status();
                        }
                        if ui.button("Status").clicked() {
                            self.show_status = !self.show_status;
                            self.show_channels = false;
                            self.model.close_status();
                        }
                        if ui.button("Channels").clicked() {
                            self.show_channels = !self.show_channels;
                            self.show_status = false;
                            self.show_settings = false;
                            self.model.close_status();
                        }
                        if ui.button("Settings").clicked() {
                            self.show_settings = !self.show_settings;
                            self.show_channels = false;
                            self.show_status = false;
                            self.model.close_status();
                        }
                        ui.label(egui::RichText::new(&self.model.status).small().weak());
                    });
                });
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.model.search)
                        .hint_text("Search")
                        .desired_width(f32::INFINITY),
                );
                ui.separator();

                let rows: Vec<ChatRow> = self.model.visible_chats().into_iter().cloned().collect();

                let previews: Vec<(String, String, String, i64)> = rows
                    .iter()
                    .map(|row| {
                        (
                            self.model.title(row),
                            self.model.row_stamp(row),
                            self.model.row_preview(row),
                            row.unread,
                        )
                    })
                    .collect();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (row, (title, stamp, preview, unread)) in rows.iter().zip(previews) {
                            let selected = self.model.active.as_deref() == Some(row.jid.as_str());
                            let text = format!(
                                "{}{}\n{}{}",
                                if selected { "▶ " } else { "" },
                                title,
                                preview,
                                if stamp.is_empty() {
                                    String::new()
                                } else {
                                    format!("   · {stamp}")
                                }
                            );
                            let label = if unread > 0 {
                                format!("{text}   ({unread})")
                            } else {
                                text
                            };
                            if ui
                                .add_sized(
                                    [ui.available_width(), 46.0],
                                    egui::Button::new(label).selected(selected),
                                )
                                .clicked()
                            {
                                let jid = row.jid.clone();
                                self.model.open_chat(&jid);
                                self.send(Command::MarkRead { chat: jid.clone() });
                            }
                        }
                    });
            });
    }

    fn conversation(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(jid) = self.model.active.clone() else {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Pick a chat").weak());
                });
                return;
            };
            let Some(row) = self.model.chats.get(&jid).cloned() else {
                return;
            };

            ui.horizontal(|ui| {
                ui.heading(self.model.title(&row));
                if row.jid.ends_with("@g.us") {
                    ui.label(egui::RichText::new("group").small().weak());
                    if ui.button("Group info").clicked() {
                        self.show_group = true;
                        if self.model.request_group(&row.jid) {
                            self.send(Command::GroupInfo {
                                chat: row.jid.clone(),
                            });
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Search").clicked() {
                        self.model.search_open = !self.model.search_open;
                        if !self.model.search_open {
                            self.model.chat_search.clear();
                            self.model.search_hit = -1;
                        }
                    }
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
                            .weak(),
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

            let height = ui.available_height() - 40.0;
            let mut chosen: Option<BubbleAction> = None;
            egui::ScrollArea::vertical()
                .stick_to_bottom(focus.is_none())
                .max_height(height.max(120.0))
                .show(ui, |ui| {
                    let ctx = ui.ctx().clone();
                    for (index, message) in row.messages.iter().enumerate() {
                        let is_hit = hits.contains(&index);
                        if let Some(action) = bubble(
                            ui,
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

            ui.horizontal(|ui| {
                if ui.button("Emoji").clicked() {
                    self.model.emoji_open = !self.model.emoji_open;
                    self.model.stickers_open = false;
                }
                if ui.button("Sticker").clicked() {
                    self.model.stickers_open = !self.model.stickers_open;
                    self.model.emoji_open = false;
                    if self.model.favorites.is_empty() && self.model.favorites_error.is_none() {
                        self.send(Command::FavoriteStickers);
                    }
                }
                let send_width = 64.0;
                let edit = ui.add_sized(
                    [ui.available_width() - send_width, 30.0],
                    egui::TextEdit::singleline(&mut self.model.composer)
                        .hint_text("Type a message"),
                );
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
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui
                    .add_sized(
                        [send_width, 30.0],
                        egui::Button::new(self.model.send_label()),
                    )
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
fn bubble(
    ui: &mut egui::Ui,
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
    ui.with_layout(egui::Layout::top_down(align), |ui| {
        let mut header = String::new();
        if let Some(name) = message.sender_name.as_ref().filter(|_| !message.from_me) {
            header.push_str(name);
            header.push_str(" · ");
        }
        header.push_str(&model.bubble_stamp(message));
        ui.label(egui::RichText::new(header).small().weak());

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
            context_menu(&response, model, message, &mut action);
            pills_row(ui, model, message);
            ui.add_space(3.0);
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
            context_menu(&response, model, message, &mut action);
            if !message.text.trim().is_empty() {
                ui.label(egui::RichText::new(&message.text).color(egui::Color32::from_gray(235)));
            }
            pills_row(ui, model, message);
            ui.add_space(3.0);
            return;
        }

        let body = model.bubble_body(message);
        let fill = if message.from_me {
            egui::Color32::from_rgb(20, 77, 55)
        } else {
            egui::Color32::from_rgb(36, 38, 38)
        };
        let mut frame = egui::Frame::new()
            .fill(fill)
            .inner_margin(egui::Margin::symmetric(8, 5))
            .corner_radius(egui::CornerRadius::same(8));
        if search.is_some() {
            frame = frame.stroke(egui::Stroke::new(
                1.5_f32,
                egui::Color32::from_rgb(240, 200, 60),
            ));
        }
        let response = frame
            .show(ui, |ui| {
                let color = egui::Color32::from_gray(240);
                match search {
                    Some(needle) => {
                        ui.label(highlight_job(&body, needle, color));
                    }
                    None => {
                        ui.label(egui::RichText::new(body).color(color));
                    }
                }
            })
            .response;
        if scroll {
            response.scroll_to_me(Some(egui::Align::Center));
        }
        // A frame only senses hover, so give its rect a click sense of its own
        // before asking for the context menu.
        let hit = ui.interact(
            response.rect,
            response.id.with("msg-menu"),
            egui::Sense::click(),
        );
        context_menu(&hit, model, message, &mut action);
        pills_row(ui, model, message);
        ui.add_space(3.0);
    });
    action
}

/// WhatsApp's message menu: the quick reaction bar, then the rows `wa-ui`
/// allows for this message.
fn context_menu(
    response: &egui::Response,
    model: &Model,
    message: &MessageView,
    action: &mut Option<BubbleAction>,
) {
    response.context_menu(|ui| {
        ui.horizontal(|ui| {
            for emoji in ["👍", "❤️", "😂", "😮", "😢", "🙏"] {
                if ui
                    .add(egui::Button::new(egui::RichText::new(emoji).size(18.0)).frame(false))
                    .clicked()
                {
                    *action = Some(BubbleAction::React(message.id.clone(), emoji.to_string()));
                    ui.close();
                }
            }
        });
        ui.separator();
        let menu = model.message_menu(message);
        if menu.reply && ui.button("Reply").clicked() {
            *action = Some(BubbleAction::Reply(message.id.clone()));
            ui.close();
        }
        if menu.copy && ui.button("Copy").clicked() {
            ui.ctx().copy_text(message.text.clone());
            ui.close();
        }
        if menu.forward && ui.button("Forward").clicked() {
            *action = Some(BubbleAction::Forward(message.id.clone()));
            ui.close();
        }
        if menu.edit && ui.button("Edit").clicked() {
            *action = Some(BubbleAction::Edit(message.id.clone()));
            ui.close();
        }
        ui.separator();
        if ui.button("Delete").clicked() {
            *action = Some(BubbleAction::Delete(message.id.clone()));
            ui.close();
        }
    });
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
                                    .color(egui::Color32::RED),
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
                ui.label(egui::RichText::new(err).color(egui::Color32::RED));
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
        self.model.now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.drain_events(ctx);
        self.model.now = now_ms();
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
