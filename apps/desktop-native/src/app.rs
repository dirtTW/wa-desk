//! The window: a chat list on the left, the open conversation on the right.
//!
//! Painting is all this module does. Every string it shows comes from
//! `crate::model`, which in turn asks `wa-ui`.

use std::sync::mpsc::Sender;

use tokio::sync::mpsc::UnboundedReceiver;
use wa_types::{MessageView, UiEvent};

use crate::login::LoginState;
use crate::model::{ChatRow, Command, Model};

/// What `main` assembles before the window exists.
pub struct NativeApp {
    pub events: UnboundedReceiver<UiEvent>,
    pub commands: Sender<Command>,
    pub runtime: tokio::runtime::Handle,
    /// Results of media downloads, (message id, cached path or error).
    pub media: std::sync::mpsc::Receiver<(String, Result<String, String>)>,
}

impl NativeApp {
    pub fn new(
        events: UnboundedReceiver<UiEvent>,
        commands: Sender<Command>,
        runtime: tokio::runtime::Handle,
        media: std::sync::mpsc::Receiver<(String, Result<String, String>)>,
    ) -> Self {
        Self {
            events,
            commands,
            runtime,
            media,
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
    /// Rasterised QR code, keyed by the SVG it came from so a rotating code
    /// is decoded once per rotation.
    qr: Option<(String, egui::TextureHandle)>,
    /// Media download results, drained every frame.
    media_rx: std::sync::mpsc::Receiver<(String, Result<String, String>)>,
    /// Decoded media textures, keyed by message id.
    textures: std::collections::HashMap<String, Option<egui::TextureHandle>>,
}

impl WaDesk {
    pub fn new(_cc: &eframe::CreationContext<'_>, native: NativeApp) -> Self {
        Self {
            model: Model::new(),
            events: native.events,
            commands: native.commands,
            runtime: native.runtime,
            typing_sent: false,
            show_status: false,
            qr: None,
            media_rx: native.media,
            textures: std::collections::HashMap::new(),
        }
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            self.model.apply(&event);
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
                        if ui.button("Status").clicked() {
                            self.show_status = !self.show_status;
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
                }
            });
            ui.separator();

            // Ask for photos that are not on disk yet (one request each).
            let wanted: Vec<String> = row
                .messages
                .iter()
                .filter(|message| message.kind == "image")
                .map(|message| message.id.clone())
                .collect();
            for id in wanted {
                if self.model.request_media(&id) {
                    self.send(Command::FetchMedia { id });
                }
            }

            let height = ui.available_height() - 54.0;
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height(height.max(120.0))
                .show(ui, |ui| {
                    let ctx = ui.ctx().clone();
                    for message in &row.messages {
                        bubble(ui, &self.model, message, &mut self.textures, &ctx);
                    }
                });

            ui.separator();
            ui.horizontal(|ui| {
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
                    .add_sized([send_width, 30.0], egui::Button::new("Send"))
                    .clicked()
                    || enter
                {
                    let text = self.model.composer.trim().to_string();
                    if !text.is_empty() {
                        self.send(Command::SendText {
                            chat: jid.clone(),
                            text: text.clone(),
                        });
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
}

fn bubble(
    ui: &mut egui::Ui,
    model: &Model,
    message: &MessageView,
    textures: &mut std::collections::HashMap<String, Option<egui::TextureHandle>>,
    ctx: &egui::Context,
) {
    let align = if message.from_me {
        egui::Align::Max
    } else {
        egui::Align::Min
    };
    ui.with_layout(egui::Layout::top_down(align), |ui| {
        let mut header = String::new();
        if let Some(name) = message.sender_name.as_ref().filter(|_| !message.from_me) {
            header.push_str(name);
            header.push_str(" · ");
        }
        header.push_str(&model.bubble_stamp(message));
        ui.label(egui::RichText::new(header).small().weak());

        // A photo paints its bytes once wa-core has the file; anything else
        // falls back to the caption or the kind's word from wa-ui.
        if message.kind == "image" {
            if !textures.contains_key(&message.id) {
                textures.insert(message.id.clone(), decode(ctx, model, message));
            }
            if let Some(Some(texture)) = textures.get(&message.id) {
                let size = texture.size_vec2();
                let scale = (240.0 / size.x).min(1.0);
                ui.image((texture.id(), size * scale));
                if !message.text.trim().is_empty() {
                    ui.label(
                        egui::RichText::new(&message.text).color(egui::Color32::from_gray(235)),
                    );
                }
                ui.add_space(3.0);
                return;
            }
        }

        let body = model.bubble_body(message);
        let fill = if message.from_me {
            egui::Color32::from_rgb(20, 77, 55)
        } else {
            egui::Color32::from_rgb(36, 38, 38)
        };
        egui::Frame::new()
            .fill(fill)
            .inner_margin(egui::Margin::symmetric(8, 5))
            .corner_radius(egui::CornerRadius::same(8))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(body).color(egui::Color32::from_gray(240)));
            });
        ui.add_space(3.0);
    });
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
            self.sidebar(ctx);
            if self.show_status {
                self.status_panel(ctx);
            } else {
                self.conversation(ctx);
            }
        }
        let _ = &self.runtime;
    }
}

/// Decode a cached media file into a texture: absent while downloading, and
/// absent when the file is not a readable image.
fn decode(
    ctx: &egui::Context,
    model: &Model,
    message: &MessageView,
) -> Option<egui::TextureHandle> {
    let path = match model.media.get(&message.id) {
        Some(crate::model::MediaState::Path(path)) => path,
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture(
        format!("media-{}", message.id),
        color,
        egui::TextureOptions::LINEAR,
    ))
}
